<script lang="ts">
	import { version } from '$app/env';
	import { OpenBrowser } from '#lib/browser.svelte.ts';
	import Browser from '#lib/shell/Browser.svelte';
	import { ago, logos } from '#lib/chat/sessions.ts';
	import { same, subagents } from '#lib/chat/agents.ts';
	import Html from '#lib/chat/Html.svelte';
	import { setHtmlPreview, type HtmlItem } from '#lib/chat/html.ts';
	import { hostName, rigs, type Rig } from '#lib/config.svelte.ts';
	import {
		connect,
		tabOf,
		type Attention,
		type Chat,
		type Epic,
		type Item,
		type Project,
		type Reading,
		type ResetCredits,
		type ServerMessage,
		type Session,
		type Terminal
	} from '#lib/device.svelte.ts';
	import { LanguageServers } from '#lib/editor/lsp.svelte.ts';
	import { heldBy, OpenEpic } from '#lib/epics.svelte.ts';
	import { image, invalidate, listing, OpenConfig, OpenDiff, OpenFile, type Cursor } from '#lib/files.svelte.ts';
	import { form } from '#lib/form.ts';
	import { status, type Change, type Commit, type Git } from '#lib/git.ts';
	import { capture, putLayout, restore, saveLayout, sendClosing } from '#lib/layout.ts';
	import { afterPaint, warm, warmUp } from '#lib/paint.svelte.ts';
	import { labels, worktrees } from '#lib/projects.ts';
	import AttentionList from '#lib/shell/Attention.svelte';
	import Settings from '#lib/shell/Settings.svelte';
	import Drawer from '#lib/shell/Drawer.svelte';
	import Leader from '#lib/shell/Leader.svelte';
	import type { Action, Scope } from '#lib/shell/Palette.svelte';
	import Projects from '#lib/shell/Projects.svelte';
	import Rail from '#lib/shell/Rail.svelte';
	import Sidebar, { showing as sectionShowing, type Section } from '#lib/shell/Sidebar.svelte';
	import TerminalView from '#lib/shell/TerminalView.svelte';
	import Usage from '#lib/shell/Usage.svelte';
	import { arrange, neighbor, type Side, type Split } from '#lib/split.ts';
	import Badge from '#lib/ui/Badge.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import Tab from '#lib/ui/Tab.svelte';
	import { onMount } from 'svelte';
	import { on } from 'svelte/events';
	import CaretLeft from '~icons/ph/caret-left';
	import CaretRight from '~icons/ph/caret-right';
	import FileCode from '~icons/ph/file-code';
	import Globe from '~icons/ph/globe';
	import Plus from '~icons/ph/plus';
	import Power from '~icons/ph/power';
	import SidebarSimple from '~icons/ph/sidebar-simple';
	import TerminalWindow from '~icons/ph/terminal-window';
	import WifiSlash from '~icons/ph/wifi-slash';

	const RECENT = 5;
	const editor = () => import('#lib/shell/Editor.svelte');
	afterPaint(editor);
	const restored = restore();
	const device = connect();
	let projects = $state<Project[]>([]);
	let terminals = $state<Record<string, Terminal[]>>({});
	let chats = $state<Record<string, Chat[]>>({});
	let chatItems = $state<Record<string, Item[]>>({});
	let htmlPreview = $state<HtmlItem>();
	let htmlDialog = $state<HTMLDialogElement>();
	setHtmlPreview((item) => htmlPreview = item);
	let openSubagents = $state<Record<string, string[]>>({});
	let sessions = $state<Record<string, Session[]>>({});
	let readingSessions = $state<Record<string, boolean>>({});
	let attentionList = $state<Attention[]>([]);
	let usage = $state<Reading[]>([]);
	let codexResets = $state<ResetCredits | null>(null);
	let visible = $state(document.visibilityState === 'visible');
	let files = $state<Record<string, OpenFile[]>>(restored.files);
	let browsers = $state<Record<string, OpenBrowser[]>>(restored.browsers);
	let epicTabs = $state<Record<string, OpenEpic[]>>(restored.epics);
	let epics = $state<Record<string, Epic[]>>({});
	const subscribed = new Set<string>();
	let git = $state<Record<string, Git>>({});
	let chosen = $state(restored.chosen);
	let chosenTabs = $state<Record<string, string>>(restored.chosenTabs);
	let splits = $state<Record<string, Split>>(restored.splits);
	let dragging = $state<{ id: string; x: number; y: number; moved: boolean; over?: Side }>();
	let dragged = false;
	let main: HTMLElement;
	let sidebar = $state(restored.sidebar?.open ?? form() === 'desktop');
	let section = $state<Section>(restored.sidebar?.section ?? 'files');
	let closing = $state<OpenFile>();
	let closingProject = $state<string>();
	let closingChat = $state<Chat>();
	const closingTicket = $derived(closingChat && heldBy(epics[closingChat.projectId] ?? [], closingChat.id));
	let picking = $state(false);
	let palette = $state<Scope>();
	let tabTarget = $state<string>();
	let afterPalette: (() => void) | undefined;
	let unfocused = $state(false);
	let directories = $state<string[]>([]);
	let screen = $state(1);
	let screens: HTMLElement | undefined;
	let held = $state(false);
	let drawer: Drawer;
	let drawerOpen = $state(false);
	let settle: ((error?: string) => void) | undefined;
	let updateAvailable = $state(false);
	const dirty = $derived(Object.values(files).flat().some((file) => file.dirty));
	const servers = new LanguageServers(
		device,
		(projectId) => projects.find((project) => project.id === projectId)?.path,
		(projectId, path) => openFile(projectId, path)
	);

	const projectLabels = $derived(labels(projects));
	const ownProjects = $derived(projects.filter((project) => !project.worktree));
	const allChats = $derived(Object.values(chats).flat());
	const nested = $derived(worktrees(projects, epics, allChats));
	const focused = $derived(projects.find((project) => project.id === chosen) ?? projects[0]);
	const tabs = $derived((focused && terminals[focused.id]) || []);
	const chatTabs = $derived((focused && chats[focused.id]) || []);
	const fileTabs = $derived((focused && files[focused.id]) || []);
	const browserTabs = $derived((focused && browsers[focused.id]) || []);
	const openEpics = $derived((focused && epicTabs[focused.id]) || []);
	const tabIds = $derived([
		...tabs.map((tab) => tab.id),
		...chatTabs.map((chat) => chat.id),
		...browserTabs.map((browser) => browser.id),
		...openEpics.map((epic) => epic.id),
		...fileTabs.map((file) => file.id)
	]);
	const activeTab = $derived(tabIds.find((id) => id === chosenTabs[focused?.id ?? '']) ?? tabIds.at(-1));
	const whole = $derived({ tabs: tabIds, active: activeTab });
	const layout = $derived(focused && arrange(splits[focused.id], tabIds, activeTab));
	const side: Side = $derived(layout && layout.right.active === activeTab ? 'right' : 'left');
	const split = $derived(form() === 'desktop' ? layout : undefined);
	const groups = $derived(split ? [split.left, split.right] : [whole]);
	const phone = $derived(form() === 'phone');
	const foreground = $derived(visible && (!phone || (screen === 1 && !drawerOpen)));
	const shownChats = $derived(foreground ? chatTabs.filter((chat) => showing(chat.id)).map((chat) => chat.id) : []);
	const shownEpics = $derived(foreground ? openEpics.filter((epic) => showing(epic.id)).map((epic) => epic.slug) : []);
	const label = $derived(focused && projectLabels.get(focused.id));
	const tabActions: Action[] = $derived.by(() => {
		const items = tabTarget === 'drawer'
			? [{ label: 'Terminal', icon: TerminalWindow, onselect: () => drawer.newTab() }]
			: tabTarget ? newTab(tabTarget) : [];
		return items.flatMap((entry) => {
			const group = 'heading' in entry ? entry.heading : 'Tabs';
			const items = 'items' in entry ? entry.items : [entry];
			return items.map((item) => ({
				...item,
				group,
				run: () => {
					afterPalette = item.onselect;
					palette = undefined;
				}
			}));
		});
	});
	const titles = $derived([
		{ title: 'Projects', sub: hostName() ?? undefined },
		{ title: label ?? 'Tabs' },
		{ title: sectionShowing(section, focused && git[focused.id]).label, sub: label }
	]);
	const deviceLayout = $derived(
		capture({ chosen, terminals, chats, files, browsers, epics: epicTabs, chosenTabs, splits, sidebar: { open: sidebar, section } })
	);

	$effect(() => device.send({ type: 'device.view', chatIds: shownChats, epics: shownEpics }));
	$effect(() => saveLayout(deviceLayout));

	const query = new URLSearchParams(location.search);
	const [queryProject, queryTab] = [query.get('project'), query.get('tab')];
	if (queryProject && queryTab) attend(queryProject, queryTab);

	onMount(() => {
		const unlisten = device.listen({ message });
		const notified = ({ data }: MessageEvent<Attention>) => attend(data.projectId, tabOf(data));
		navigator.serviceWorker?.addEventListener('message', notified);
		return () => {
			unlisten();
			navigator.serviceWorker?.removeEventListener('message', notified);
			device.close();
		};
	});

	function message(message: ServerMessage) {
		servers.message(message);
		switch (message.type) {
			case 'hello': {
				readingSessions = {};
				updateAvailable = (message.buildId ?? message.version) !== version;
				const owned: Record<string, Terminal[]> = {};
				for (const terminal of message.terminals) {
					if (terminal.owner !== 'drawer') (owned[terminal.owner.project] ??= []).push(terminal);
				}
				terminals = owned;
				const chatsOwned: Record<string, Chat[]> = {};
				for (const chat of message.chats) (chatsOwned[chat.projectId] ??= []).push(chat);
				chats = chatsOwned;
				for (const project of projects) invalidate(project.id);
				subscribed.clear();
				device.send({ type: 'projects.list' });
				device.send({ type: 'device.view', chatIds: shownChats, epics: shownEpics });
				break;
			}
			case 'attention.changed':
				attentionList = message.items;
				break;
			case 'usage.changed':
				usage = message.readings;
				codexResets = message.codexResets ?? null;
				break;
			case 'terminals.changed':
				if (message.projectId) terminals[message.projectId] = message.terminals;
				break;
			case 'terminal.created': {
				const { owner, id } = message.terminal;
				if (owner !== 'drawer') {
					terminals[owner.project] ??= [];
					const owned = terminals[owner.project];
					if (!owned.some((terminal) => terminal.id === id)) owned.push(message.terminal);
					place(owner.project, id);
					choose(owner.project, id);
					if (owner.project === focused?.id) slide(1);
				}
				break;
			}
			case 'chats.changed': {
				const selected = activeTab;
				if (selected && focused?.worktree?.parent === message.projectId && message.chats.some(({ id }) => id === selected)) {
					chosenTabs[message.projectId] = selected;
				}
				const listed = (chat: Chat) => message.chats.some(({ id }) => id === chat.id);
				const gone = (chats[message.projectId] ?? []).filter((chat) => !listed(chat));
				for (const projectId in chats) {
					if (projectId !== message.projectId && chats[projectId].some(listed)) chats[projectId] = chats[projectId].filter((chat) => !listed(chat));
				}
				chats[message.projectId] = message.chats;
				for (const chat of gone) {
					if (Object.values(chats).flat().some(({ id }) => id === chat.id)) continue;
					delete chatItems[chat.id];
					delete openSubagents[chat.id];
				}
				break;
			}
			case 'chat.status':
				for (const chat of Object.values(chats).flat()) {
					if (chat.id === message.chatId) chat.status = message.status;
				}
				break;
			case 'chat.activity':
				for (const chat of Object.values(chats).flat()) {
					if (chat.id === message.chatId) chat.activity = message.activity;
				}
				break;
			case 'chat.items': {
				chatItems[message.chatId] = message.items;
				const opened = openSubagents[message.chatId];
				if (opened && !subagents(message.items).some(({ path }) => same(path, opened))) delete openSubagents[message.chatId];
				break;
			}
			case 'chat.item':
				(chatItems[message.chatId] ??= [])[message.index] = message.item;
				break;
			case 'chat.meters':
				for (const chat of Object.values(chats).flat()) {
					if (chat.id !== message.chatId) continue;
					chat.context = message.context;
					chat.tokens = message.tokens;
				}
				break;
			case 'chat.todos':
				for (const chat of Object.values(chats).flat()) {
					if (chat.id === message.chatId) chat.todos = message.todos;
				}
				break;
			case 'chat.queued':
				for (const chat of Object.values(chats).flat()) {
					if (chat.id === message.chatId) chat.queued = message.queued;
				}
				break;
			case 'chat.settings':
				for (const chat of Object.values(chats).flat()) {
					if (chat.id === message.chatId) chat.settings = message.settings;
				}
				break;
			case 'chat.created': {
				chats[message.chat.projectId] ??= [];
				const owned = chats[message.chat.projectId];
				if (!owned.some((chat) => chat.id === message.chat.id)) owned.push(message.chat);
				place(message.chat.projectId, message.chat.id);
				choose(message.chat.projectId, message.chat.id);
				if (message.chat.projectId === focused?.id) slide(1);
				break;
			}
			case 'sessions.listed':
				sessions[message.projectId] = message.sessions;
				readingSessions[message.projectId] = false;
				break;
			case 'terminal.title':
				for (const terminal of Object.values(terminals).flat()) {
					if (terminal.id === message.terminalId) terminal.label = message.label;
				}
				break;
			case 'projects.changed': {
				const left = focused;
				const selected = activeTab;
				projects = message.projects;
				if (left?.worktree && !projects.some((project) => project.id === left.id)) {
					const { parent } = left.worktree;
					chosenTabs[parent] = selected ?? chosenTabs[parent];
					chosen = parent;
				}
				for (const projectId in browsers) {
					if (!projects.some((project) => project.id === projectId)) delete browsers[projectId];
				}
				for (const projectId in files) {
					if (!projects.some((project) => project.id === projectId)) delete files[projectId];
				}
				for (const projectId in git) {
					if (!projects.some((project) => project.id === projectId)) delete git[projectId];
				}
				for (const projectId in splits) {
					if (!projects.some((project) => project.id === projectId)) delete splits[projectId];
				}
				for (const projectId in chosenTabs) {
					if (!projects.some((project) => project.id === projectId)) delete chosenTabs[projectId];
				}
				for (const projectId in epicTabs) {
					if (!projects.some((project) => project.id === projectId)) delete epicTabs[projectId];
				}
				for (const projectId in epics) {
					if (!projects.some((project) => project.id === projectId)) delete epics[projectId];
				}
				for (const projectId of subscribed) {
					if (!projects.some((project) => project.id === projectId)) subscribed.delete(projectId);
				}
				for (const { id } of projects) {
					if (subscribed.has(id)) continue;
					subscribed.add(id);
					device.send({ type: 'epics.subscribe', projectId: id });
				}
				for (const project of projects) loadGit(project.id);
				if (!activeTab) warmUp();
				break;
			}
			case 'git.changed':
				loadGit(message.projectId);
				break;
			case 'epics.changed':
				closeArchived(message.projectId, message.epics);
				epics[message.projectId] = message.epics;
				break;
			case 'epic.changed': {
				const { epic } = message;
				closeArchived(message.projectId, [epic]);
				const listed = (epics[message.projectId] ??= []);
				const index = listed.findIndex((other) => other.slug >= epic.slug);
				if (listed[index]?.slug === epic.slug) listed[index] = epic;
				else listed.splice(index === -1 ? listed.length : index, 0, epic);
				break;
			}
			case 'ticket.started':
			case 'ticket.error':
				for (const open of epicTabs[message.projectId] ?? []) {
					if (open.slug === message.epic) open.settle(message.ticket, message.type === 'ticket.error' ? message.error : undefined);
				}
				break;
			case 'fs.changed':
				invalidate(message.projectId);
				break;
			case 'picker.listed':
				directories = message.directories;
				break;
			case 'project.opened':
				focus(message.projectId);
				picking = false;
				settle?.();
				break;
			case 'project.error':
				settle?.(message.error);
				break;
		}
	}

	function attend(projectId: string, tabId: string) {
		if (drawerOpen) drawer.toggle();
		focus(projectId);
		const [kind, , slug] = tabId.split(':');
		if (kind === 'epic' && slug) openEpic(projectId, slug);
		else choose(projectId, tabId);
	}

	function focus(projectId: string) {
		chosen = projectId;
		slide(1);
	}

	function slide(index: number) {
		screen = index;
		screens?.scrollTo({ left: index * screens.clientWidth });
	}

	function swipeable(element: HTMLElement) {
		screens = element;
		screen = 1;
		element.scrollTo({ left: element.clientWidth, behavior: 'instant' });
		const release = () => (held = false);
		const off = [
			on(element, 'touchstart', hold, { passive: true }),
			on(element, 'touchend', release),
			on(element, 'touchcancel', release)
		];
		return () => {
			off.forEach((listener) => listener());
			screens = undefined;
		};
	}

	// overscroll-behavior would also trap swipes on scrollers that fit, so the screens stop scrolling instead.
	function hold({ target }: TouchEvent) {
		for (let element = target as Element | null; element && element !== screens; element = element.parentElement) {
			if (element.scrollWidth > element.clientWidth && /auto|scroll/.test(getComputedStyle(element).overflowX)) {
				held = true;
				return;
			}
		}
	}

	function landed() {
		if (screens) screen = Math.round(screens.scrollLeft / screens.clientWidth);
	}

	async function loadGit(projectId: string) {
		const loaded = await status(projectId).catch(() => undefined);
		if (loaded && projects.some((project) => project.id === projectId)) git[projectId] = loaded;
	}

	function pick() {
		directories = [];
		picking = true;
		device.send({ type: 'picker.list' });
	}

	function open(path: string): Promise<string | undefined> {
		settle?.();
		device.send({ type: 'project.open', path });
		return new Promise((resolve) => (settle = resolve));
	}

	function reorder(order: string[]) {
		projects = [...order.flatMap((id) => projects.filter((project) => project.id === id)), ...projects.filter((project) => project.worktree)];
		device.send({ type: 'project.reorder', order });
	}

	function closeProject(projectId: string, confirmed = false) {
		const agents = allChats.filter((chat) => chat.projectId === projectId || projects.some(({ id, worktree }) => id === chat.projectId && worktree?.parent === projectId));
		if (!confirmed && agents.some((agent) => agent.status === 'working')) {
			closingProject = projectId;
			return;
		}
		closingProject = undefined;
		device.send({ type: 'project.close', projectId });
	}

	function readSessions(projectId: string) {
		readingSessions[projectId] = true;
		device.send({ type: 'sessions.list', projectId });
	}

	function newTab(projectId: string) {
		const items = rigs()
			.map((rig, index) => ({ label: rig.name, hint: index < 9 ? `Ctrl+Shift+${index + 1}` : `${index + 1}`, icon: logos[rig.agent], onselect: () => launch(rig, projectId) }));
		const terminal = () => device.send({ type: 'terminal.create', projectId });
		const rest = [
			{ label: 'Terminal', hint: 'Ctrl+Shift+T', icon: TerminalWindow, onselect: terminal },
			{ label: 'Browser', icon: Globe, onselect: () => openBrowser(projectId) },
			{ label: 'Open file', icon: FileCode, onselect: () => openPalette('files') }
		];
		const recent = (sessions[projectId] ?? []).slice(0, RECENT).map((session) => ({
			label: session.firstPrompt,
			icon: logos[session.agent],
			hint: ago(session.updatedAt),
			onselect: () => reopen(projectId, session)
		}));
		const past = recent.length
			? [{ heading: 'Sessions', items: [...recent, { label: 'Search sessions…', onselect: () => (palette = 'sessions') }] }]
			: [];
		return [...(items.length ? [{ heading: 'Rigs', items }] : []), ...rest, ...past];
	}

	function reopen(projectId: string, { agent, sessionId }: Session) {
		device.send({ type: 'chat.resume', projectId, agent, sessionId });
	}

	function launch(rig: Rig, projectId = focused?.id) {
		if (projectId) device.send({ type: 'chat.create', projectId, rig: rig.name });
	}

	function jump(index: number) {
		const tabId = groups.flatMap((group) => group.tabs)[index];
		if (focused && tabId) choose(focused.id, tabId);
	}

	function openPalette(scope: Scope) {
		if (focused) listing(focused.id).refresh();
		palette = scope;
	}

	function choose(projectId: string, tabId: string) {
		chosenTabs[projectId] = tabId;
		show(projectId, tabId);
	}

	function show(projectId: string, tabId: string) {
		const stored = splits[projectId];
		if (stored) (stored.right.tabs.includes(tabId) ? stored.right : stored.left).active = tabId;
	}

	function showing(tabId: string) {
		return groups.some((group) => group.active === tabId);
	}

	function column(tabId: string) {
		return split?.right.tabs.includes(tabId) ? 3 : 1;
	}

	function place(projectId: string, tabId: string) {
		if (projectId === focused?.id && side === 'right') splits[projectId].right.tabs.push(tabId);
	}

	function move(tabId: string, to: Side) {
		if (!focused) return;
		const source = layout ? layout[to === 'left' ? 'right' : 'left'] : whole;
		if (!source.tabs.includes(tabId)) return;
		const rest = source.tabs.filter((id) => id !== tabId);
		if (rest.length) {
			const kept = { tabs: rest, active: source.active === tabId ? neighbor(source.tabs, tabId) : source.active };
			const moved = { tabs: [...(layout?.[to].tabs ?? []), tabId], active: tabId };
			const [left, right] = to === 'left' ? [moved, kept] : [kept, moved];
			splits[focused.id] = { left, right, ratio: layout?.ratio ?? 0.5 };
		} else delete splits[focused.id];
		choose(focused.id, tabId);
	}

	function grab(event: PointerEvent) {
		const id = (event.target as HTMLElement).closest<HTMLElement>('[data-tab]')?.dataset.tab;
		if (!id || event.button !== 0 || form() !== 'desktop') return;
		dragging = { id, x: event.clientX, y: event.clientY, moved: false };
		dragged = false;
		window.addEventListener('pointermove', drag);
		window.addEventListener('pointerup', drop);
		window.addEventListener('pointercancel', drop);
	}

	function drag(event: PointerEvent) {
		if (!dragging || (!dragging.moved && Math.hypot(event.clientX - dragging.x, event.clientY - dragging.y) < 4)) return;
		dragging.moved = true;
		const { left, right, top, bottom, width } = main.getBoundingClientRect();
		const inside = event.clientX >= left && event.clientX < right && event.clientY >= top && event.clientY < bottom;
		dragging.over = inside ? (event.clientX - left < width * (split?.ratio ?? 0.5) ? 'left' : 'right') : undefined;
	}

	function drop(event: PointerEvent) {
		window.removeEventListener('pointermove', drag);
		window.removeEventListener('pointerup', drop);
		window.removeEventListener('pointercancel', drop);
		if (dragging?.moved) {
			dragged = true;
			setTimeout(() => (dragged = false));
			if (event.type === 'pointerup' && dragging.over) move(dragging.id, dragging.over);
		}
		dragging = undefined;
	}

	function resize(event: PointerEvent & { currentTarget: HTMLElement }) {
		if (!focused || !event.currentTarget.hasPointerCapture(event.pointerId)) return;
		const { left, width } = main.getBoundingClientRect();
		splits[focused.id].ratio = Math.min(Math.max((event.clientX - left) / width, 0.25), 0.75);
	}

	function leave(projectId: string, tabId: string) {
		const halves = layout ? [layout.left, layout.right] : [whole];
		const group = halves.find((group) => group.tabs.includes(tabId));
		if (!group) return;
		if (group.tabs.length === 1) delete splits[projectId];
		if (tabId !== group.active) return;
		const next = neighbor(group.tabs, tabId) ?? halves.find((other) => other !== group)?.active;
		if (next && tabId === activeTab) choose(projectId, next);
		else if (next) show(projectId, next);
	}

	function closeTab(projectId: string, terminalId: string) {
		leave(projectId, terminalId);
		device.send({ type: 'terminal.close', terminalId });
	}

	function openTabPicker() {
		tabTarget = drawer.ownsFocus() ? 'drawer' : focused?.id;
		if (!tabTarget) return;
		if (tabTarget !== 'drawer') readSessions(tabTarget);
		palette = 'tabs';
	}

	function openTerminal() {
		if (drawer.ownsFocus()) drawer.newTab();
		else if (focused) device.send({ type: 'terminal.create', projectId: focused.id });
	}

	function finishTabSelection(event: Event) {
		if (!afterPalette) return;
		event.preventDefault();
		const open = afterPalette;
		afterPalette = undefined;
		open();
	}

	function closeActiveTab() {
		if (drawer.ownsFocus()) return drawer.closeActiveTab();
		if (!focused || !activeTab) return;
		const file = fileTabs.find((file) => file.id === activeTab);
		const browser = browserTabs.find((browser) => browser.id === activeTab);
		const epic = openEpics.find((epic) => epic.id === activeTab);
		const chat = chatTabs.find((chat) => chat.id === activeTab);
		if (file) closeFile(file);
		else if (browser) closeBrowser(browser);
		else if (epic) closeEpic(epic);
		else if (chat) closeChat(chat);
		else closeTab(focused.id, activeTab);
	}

	function closeChat(chat: Chat, confirmed = false) {
		if (!confirmed && (chat.status === 'working' || heldBy(epics[chat.projectId] ?? [], chat.id))) {
			closingChat = chat;
			return;
		}
		closingChat = undefined;
		leave(chat.projectId, chat.id);
		device.send({ type: 'chat.close', chatId: chat.id });
	}

	function tabMenu(projectId: string, tab: Terminal) {
		const close = () => closeTab(projectId, tab.id);
		return [{ label: tab.state === 'live' ? 'Close' : 'Dismiss', onselect: close }];
	}

	function openTab<T extends OpenFile>(tab: T): T {
		files[tab.projectId] ??= [];
		const open = files[tab.projectId];
		const existing = open.find((file) => file.id === tab.id) as T | undefined;
		if (!existing) {
			open.push(tab);
			place(tab.projectId, tab.id);
		}
		choose(tab.projectId, tab.id);
		slide(1);
		return existing ?? tab;
	}

	function openFile(projectId: string, path: string, cursor?: Cursor) {
		const file = openTab(new OpenFile(projectId, path));
		if (cursor) file.cursor = cursor;
	}

	function openBrowser(projectId: string, url = '') {
		browsers[projectId] ??= [];
		const open = browsers[projectId];
		let browser = url ? open.find((browser) => browser.url === url) : undefined;
		if (!browser) {
			browser = new OpenBrowser(projectId);
			browser.url = url;
			open.push(browser);
			place(projectId, browser.id);
		}
		choose(projectId, browser.id);
		slide(1);
	}

	function closeBrowser(browser: OpenBrowser) {
		leave(browser.projectId, browser.id);
		browsers[browser.projectId] = browsers[browser.projectId].filter((open) => open.id !== browser.id);
	}

	function openEpic(projectId: string, slug: string) {
		epicTabs[projectId] ??= [];
		const open = epicTabs[projectId];
		let epic = open.find((epic) => epic.slug === slug);
		if (!epic) {
			epic = new OpenEpic(projectId, slug);
			open.push(epic);
			place(projectId, epic.id);
		}
		choose(projectId, epic.id);
		slide(1);
	}

	function closeEpic(epic: OpenEpic) {
		leave(epic.projectId, epic.id);
		epicTabs[epic.projectId] = epicTabs[epic.projectId].filter((open) => open !== epic);
	}

	function closeArchived(projectId: string, incoming: Epic[]) {
		const archived = incoming.filter((epic) => epic.archived && epics[projectId]?.some((previous) => previous.slug === epic.slug && !previous.archived)).map((epic) => epic.slug);
		for (const epic of (epicTabs[projectId] ?? []).filter((open) => archived.includes(open.slug))) closeEpic(epic);
	}

	function openConfig() {
		palette = undefined;
		if (focused) openTab(new OpenConfig(focused.id));
		else unfocused = true;
	}

	function openDiff(projectId: string, { path, from }: Change, against: 'HEAD' | Commit = 'HEAD') {
		openTab(new OpenDiff(projectId, path, against, from));
	}

	function showDiff(file: OpenFile, disk: string, buffer: string) {
		openTab(new OpenDiff(file.projectId, file.path, 'disk')).snapshot = { disk, buffer };
	}

	function closeFile(file: OpenFile, discard = false) {
		if (file.dirty && !discard) {
			closing = file;
			return;
		}
		closing = undefined;
		leave(file.projectId, file.id);
		files[file.projectId] = files[file.projectId].filter((open) => open !== file);
	}
</script>

<svelte:document onvisibilitychange={() => (visible = document.visibilityState === 'visible')} />

<svelte:window onpagehide={() => sendClosing(deviceLayout)} />

{#snippet badge(project: Project)}
	{@const count = attentionList.filter((item) => item.projectId === project.id).length}
	{#if count}<Badge tone="waiting">{count}</Badge>{/if}
{/snippet}

{#snippet attention()}
	<Settings />
	<AttentionList items={attentionList} projects={projectLabels} chats={allChats} onselect={(item) => attend(item.projectId, tabOf(item))} />
{/snippet}

{#snippet sidebarPanel()}
	<Sidebar
		open={phone || sidebar}
		project={focused}
		{label}
		git={focused && git[focused.id]}
		{device}
		bind:section
		{usage}
		{codexResets}
		epics={focused && epics[focused.id]}
		onopen={(path, cursor) => focused && openFile(focused.id, path, cursor)}
		ondiff={(change, commit) => focused && openDiff(focused.id, change, commit)}
		onepic={(slug) => focused && openEpic(focused.id, slug)}
	/>
{/snippet}

{#snippet workspace()}
	<main
		bind:this={main}
		style:grid-template-columns={split &&
			`minmax(0, ${split.ratio}fr) var(--border-width) minmax(0, ${1 - split.ratio}fr)`}
	>
		{#each groups as group, index (index)}
			<div
				class="strip"
				role="presentation"
				style:grid-column={index * 2 + 1}
				onfocusin={() => focused && group.active && choose(focused.id, group.active)}
				onpointerdown={grab}
			>
				{#if index === 0 && !phone}
					<IconButton label="Sidebar" aria-expanded={sidebar} onclick={() => (sidebar = !sidebar)}>
						<SidebarSimple />
					</IconButton>
				{/if}
				<div role="tablist" aria-label={split ? `${index ? 'Right' : 'Left'} Tabs` : 'Tabs'}>
					{#if focused}
						{@const projectId = focused.id}
						{#each group.tabs as id (id)}
							{@const tab = tabs.find((tab) => tab.id === id)}
							{@const chat = chatTabs.find((chat) => chat.id === id)}
							{@const file = fileTabs.find((file) => file.id === id)}
							{@const browser = browserTabs.find((browser) => browser.id === id)}
							{@const epic = openEpics.find((epic) => epic.id === id)}
							{#if tab}
								<Menu context items={tabMenu(projectId, tab)}>
									{#snippet trigger(props)}
										<Tab
											{...props}
											data-tab={id}
											variant="terminal"
											ended={tab.state === 'ended'}
											label={tab.label}
											active={id === group.active}
											onclose={() => closeTab(projectId, id)}
											onclick={() => {
												if (!dragged) choose(projectId, id);
											}}
										/>
									{/snippet}
								</Menu>
							{:else if chat}
								<Menu context items={[{ label: 'Close', onselect: () => closeChat(chat) }]}>
									{#snippet trigger(props)}
										<Tab
											{...props}
											data-tab={id}
											variant="agent"
											agent={chat.agent}
											status={chat.status === 'done' ? subagents(chatItems[chat.id] ?? []).find(({ subagent }) => subagent.status !== 'done')?.subagent.status ?? 'done' : chat.status}
											label={chat.rig}
											active={id === group.active}
											onclose={() => closeChat(chat)}
											onclick={() => {
												if (!dragged) choose(projectId, id);
											}}
										/>
									{/snippet}
								</Menu>
							{:else if browser}
								<Menu context items={[{ label: 'Close', onselect: () => closeBrowser(browser) }]}>
									{#snippet trigger(props)}
										<Tab {...props} data-tab={id} variant="browser" label={browser.name} active={id === group.active}
											onclose={() => closeBrowser(browser)} onclick={() => { if (!dragged) choose(projectId, id); }} />
									{/snippet}
								</Menu>
							{:else if epic}
								<Menu context items={[{ label: 'Close', onselect: () => closeEpic(epic) }]}>
									{#snippet trigger(props)}
										<Tab
											{...props}
											data-tab={id}
											variant="epic"
											label={epic.name}
											active={id === group.active}
											onclose={() => closeEpic(epic)}
											onclick={() => {
												if (!dragged) choose(projectId, id);
											}}
										/>
									{/snippet}
								</Menu>
							{:else if file}
								<Menu context items={[{ label: 'Close', onselect: () => closeFile(file) }]}>
									{#snippet trigger(props)}
										<Tab
											{...props}
											data-tab={id}
											variant={file instanceof OpenDiff ? 'diff' : 'editor'}
											against={file instanceof OpenDiff ? file.versus : undefined}
											label={file.name}
											title={file.deleted ? `${file.path} (deleted on disk)` : file.path}
											dirty={file.dirty}
											deleted={file.deleted}
											active={id === group.active}
											onclose={() => closeFile(file)}
											onclick={() => {
												if (!dragged) choose(projectId, id);
											}}
										/>
									{/snippet}
								</Menu>
							{/if}
						{/each}
					{/if}
				</div>
				{#if focused}
					{@const projectId = focused.id}
					<Menu items={newTab(projectId)} resize loading={readingSessions[projectId] ? 'Reading sessions…' : undefined} onopen={() => readSessions(projectId)}>
						{#snippet trigger(props)}
							<IconButton {...props} label="New Tab" title="New tab (Ctrl/Cmd+T)"><Plus /></IconButton>
						{/snippet}
					</Menu>
				{/if}
			</div>
		{/each}
		{#if split}
			<div
				class="divider"
				role="separator"
				aria-orientation="vertical"
				aria-valuemin={25}
				aria-valuemax={75}
				aria-valuenow={Math.round(split.ratio * 100)}
				onpointerdown={(event) => {
					event.preventDefault();
					event.currentTarget.setPointerCapture(event.pointerId);
				}}
				onpointermove={resize}
			></div>
		{/if}
		<div class="tabs">
			{#each Object.values(browsers).flat() as browser (browser.id)}
				<div class="view" hidden={!showing(browser.id)} style:grid-column={column(browser.id)} onfocusin={() => choose(browser.projectId, browser.id)}>
					{#if showing(browser.id) || warm()}<Browser {browser} focused={browser.id === activeTab} />{/if}
				</div>
			{/each}
			{#each Object.values(epicTabs).flat() as epic (epic.id)}
				<div class="view" hidden={!showing(epic.id)} style:grid-column={column(epic.id)} onfocusin={() => choose(epic.projectId, epic.id)}>
					{#if showing(epic.id) || warm()}
						{#await import('#lib/epics/EpicView.svelte') then { default: EpicView }}
							<EpicView
								open={epic}
								epic={epics[epic.projectId]?.find((listed) => listed.slug === epic.slug)}
								loaded={!!epics[epic.projectId]}
								active={showing(epic.id)}
								{device}
								chats={allChats}
								onchat={(chatId) => {
									const chat = allChats.find(({ id }) => id === chatId);
									if (chat) attend(chat.projectId, chatId);
								}}
							/>
						{/await}
					{/if}
				</div>
			{/each}
			{#each Object.entries(terminals) as [projectId, owned] (projectId)}
				{@const project = projects.find((project) => project.id === projectId)}
				{#each owned as terminal (terminal.id)}
					{@const ended = terminal.state === 'ended'}
					<div
						class="view"
						hidden={!showing(terminal.id)}
						style:grid-column={column(terminal.id)}
						onfocusin={() => choose(projectId, terminal.id)}
					>
						{#if showing(terminal.id) || warm()}
							<TerminalView
								{device}
								terminalId={terminal.id}
								active={showing(terminal.id)}
								focused={terminal.id === activeTab}
								{ended}
								{project}
								onopen={(path, cursor) => openFile(projectId, path, cursor)}
								onbrowse={(url) => openBrowser(projectId, url)}
							>
								{#if ended}
									<div class="ended" role="status">
										<span class="message"><Power /> Lost to a Host reboot; showing its last output.</span>
										<div class="choices">
											<Button onclick={() => closeTab(projectId, terminal.id)}>Dismiss</Button>
										</div>
									</div>
								{/if}
							</TerminalView>
						{/if}
					</div>
				{/each}
			{/each}
			{#each Object.values(chats).flat() as chat (chat.id)}
				<div
					class="view"
					hidden={!showing(chat.id)}
					style:grid-column={column(chat.id)}
					onfocusin={() => choose(chat.projectId, chat.id)}
				>
					{#if showing(chat.id) || warm()}
						{#await import('#lib/chat/ChatView.svelte') then { default: ChatView }}
							<ChatView
								{device}
								{chat}
								chats={allChats}
								onchat={(target) => { chosen = target.projectId; choose(target.projectId, target.id); slide(1); }}
								items={chatItems[chat.id] ?? []}
								opened={openSubagents[chat.id] ?? []}
								onopen={(path) => (openSubagents[chat.id] = path)}
								active={showing(chat.id)}
								focused={chat.id === activeTab}
								ondiff={(path) => openTab(new OpenDiff(chat.projectId, path, 'HEAD'))}
							/>
						{/await}
					{/if}
				</div>
			{/each}
			{#each Object.values(files) as open}
				{#each open as file (file.id)}
					{@const onshowdiff = (disk: string, buffer: string) => showDiff(file, disk, buffer)}
					{@const active = showing(file.id)}
					<div
						class="view"
						hidden={!active}
						style:grid-column={column(file.id)}
						onfocusin={() => choose(file.projectId, file.id)}
					>
						{#if active || warm()}
							{#if file instanceof OpenDiff}
								{#await import('#lib/shell/Diff.svelte') then { default: Diff }}
									<Diff diff={file} {device} {servers} {active} displayed={active && foreground} focused={file.id === activeTab} {onshowdiff} />
								{/await}
							{:else if image(file.path) && !/\.svg$/i.test(file.path)}
								{#await import('#lib/shell/ImageView.svelte') then { default: ImageView }}
									<ImageView {file} {device} {active} />
								{/await}
							{:else}
								{#await editor() then { default: Editor }}
									<Editor
										{file}
										{device}
										{servers}
										git={git[file.projectId]}
										{active}
										displayed={active && foreground}
										focused={file.id === activeTab}
										{onshowdiff}
									/>
								{/await}
							{/if}
						{/if}
					</div>
				{/each}
			{/each}
			{#if dragging?.moved}
				{@const ratio = split?.ratio ?? 0.5}
				<div class="zones">
					<div class="zone" class:over={dragging.over === 'left'} style:flex-grow={ratio}></div>
					<div class="zone" class:over={dragging.over === 'right'} style:flex-grow={1 - ratio}></div>
				</div>
			{/if}
			{#if !focused}
				<div class="empty">
					<p>No Project open</p>
					<Button variant="primary" onclick={pick}>Open Project</Button>
				</div>
			{/if}
		</div>
	</main>
{/snippet}

{#if phone}
	{@const { title, sub } = titles[screen]}
	<div class="phone">
		<header>
			<IconButton label="Previous screen" disabled={screen === 0} onclick={() => slide(screen - 1)}>
				<CaretLeft />
			</IconButton>
			<h1>{title}{#if sub}{' '}<span class="sub">{sub}</span>{/if}</h1>
			{@render attention()}
			<IconButton label="Drawer" onclick={() => drawer.toggle()}><TerminalWindow /></IconButton>
			<IconButton label="Next screen" disabled={screen === titles.length - 1} onclick={() => slide(screen + 1)}>
				<CaretRight />
			</IconButton>
		</header>
		<div class="screens" class:held {@attach swipeable} onscrollend={landed}>
			<div class="screen" inert={screen !== 0}>
				<Usage readings={usage} {codexResets} />
				<Projects
					projects={ownProjects}
					labels={projectLabels}
					worktrees={nested}
					{epics}
					focused={focused?.id}
					onfocus={focus}
					onclose={(projectId) => closeProject(projectId)}
					onpick={pick}
					{badge}
				/>
			</div>
			<div class="screen" inert={screen !== 1}>{@render workspace()}</div>
			<div class="screen" inert={screen !== 2}>{@render sidebarPanel()}</div>
		</div>
	</div>
{:else}
	<div class="shell">
		<Rail
			projects={ownProjects}
			labels={projectLabels}
			worktrees={nested}
			{epics}
			focused={focused?.id}
			onfocus={(id) => (chosen = id)}
			onreorder={reorder}
			onclose={(projectId) => closeProject(projectId)}
			onpick={pick}
			{badge}
			{attention}
		/>
		{@render sidebarPanel()}
		{@render workspace()}
	</div>
{/if}

<Drawer bind:this={drawer} bind:open={drawerOpen} {device} />

{#if htmlPreview}
	<dialog bind:this={htmlDialog} class="html-preview" aria-label={htmlPreview.title} onclose={() => htmlPreview = undefined} {@attach (node) => { node.showModal(); }}>
		<Html item={htmlPreview} full onclose={() => htmlDialog?.close()} />
	</dialog>
{/if}

{#if device.connection === 'reconnecting'}
	<div class="reconnecting" role="status"><WifiSlash /> Reconnecting…</div>
{:else if updateAvailable}
	<div class="update" role="status">
		<span>{dirty ? 'Save your edits to reload the update.' : 'Stampede updated.'}</span>
		<Button disabled={dirty} onclick={async () => { await putLayout(deviceLayout); location.reload(); }}>Reload</Button>
	</div>
{/if}

<Leader
	ontab={jump}
	onnew={openTabPicker}
	onterminal={openTerminal}
	onclose={closeActiveTab}
	onpalette={() => openPalette('all')}
	onrig={launch}
	onsplit={() => activeTab && move(activeTab, side === 'left' ? 'right' : 'left')}
/>

<Dialog
	bind:open={() => !!palette, (open) => !open && (palette = undefined)}
	oncloseautofocus={finishTabSelection}
	title={{ all: 'Command Palette', files: 'Open File', sessions: 'Past Sessions', tabs: 'New Tab' }[palette ?? 'all']}
>
	{#if palette}
		{#await import('#lib/shell/Palette.svelte') then { default: Palette }}
			{#key palette}
				<Palette
					scope={palette}
					{projects}
					labels={projectLabels}
					{focused}
					actions={palette === 'tabs' ? tabActions : [{ label: 'Open config', run: openConfig }]}
					sessions={focused && sessions[focused.id]}
					loadingSessions={!!readingSessions[palette === 'tabs' ? tabTarget ?? '' : focused?.id ?? '']}
					onproject={(projectId) => ((chosen = projectId), (palette = undefined))}
					onrig={(launcher) => (launch(launcher), (palette = undefined))}
					onfile={(path) => focused && (openFile(focused.id, path), (palette = undefined))}
					onsession={(session) => focused && (reopen(focused.id, session), (palette = undefined))}
				/>
			{/key}
		{/await}
	{/if}
</Dialog>

<Dialog bind:open={unfocused} title="Open a Project first" description="The config opens as a Tab in the Focused Project.">
	{#snippet actions()}
		<Button variant="primary" onclick={() => (unfocused = false)}>OK</Button>
	{/snippet}
</Dialog>

<Dialog bind:open={picking} full={phone} title="Open Project">
	{#await import('#lib/shell/Picker.svelte') then { default: Picker }}
		<Picker {directories} onopen={open} />
	{/await}
</Dialog>

<Dialog
	bind:open={() => !!closing, (open) => !open && (closing = undefined)}
	title="Discard unsaved changes?"
	description={closing && `${closing.path} has unsaved changes.`}
>
	{#snippet actions()}
		<Button onclick={() => (closing = undefined)}>Cancel</Button>
		<Button variant="primary" onclick={() => closing && closeFile(closing, true)}>Discard</Button>
	{/snippet}
</Dialog>

<Dialog
	bind:open={() => !!closingChat, (open) => !open && (closingChat = undefined)}
	title="Close {closingChat?.rig}?"
	description={closingTicket
		? `It holds ${closingTicket}. Closing the Chat stops its Agent and gives ${closingTicket} back to not started, keeping its worktree and session.`
		: 'Its Agent is working. Closing the Chat stops it.'}
>
	{#snippet actions()}
		<Button onclick={() => (closingChat = undefined)}>Cancel</Button>
		<Button variant="primary" onclick={() => closingChat && closeChat(closingChat, true)}>Close</Button>
	{/snippet}
</Dialog>

<Dialog
	bind:open={() => !!closingProject, (open) => !open && (closingProject = undefined)}
	title="Close {closingProject && projectLabels.get(closingProject)}?"
	description="An Agent is working in this Project. Closing it ends its Terminals."
>
	{#snippet actions()}
		<Button onclick={() => (closingProject = undefined)}>Cancel</Button>
		<Button variant="primary" onclick={() => closingProject && closeProject(closingProject, true)}>Close</Button>
	{/snippet}
</Dialog>

<style>
	.html-preview {
		inset: 0;
		width: 100%;
		max-width: none;
		height: 100%;
		max-height: none;
		margin: 0;
		padding: 0;
		overflow: hidden;
		border: 0;
		background: var(--surface-1);
		color: var(--color-fg);
	}

	.shell {
		display: flex;
		height: 100dvh;
	}

	.phone {
		display: flex;
		flex-direction: column;
		height: 100dvh;
	}

	header {
		display: flex;
		flex: none;
		align-items: center;
		gap: var(--space-1);
		padding: var(--space-1);
		border-bottom: var(--border-width) solid var(--border);
	}

	h1 {
		flex: 1;
		min-width: 0;
		margin: 0;
		overflow: hidden;
		font-size: var(--font-size-lg);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.sub {
		margin-left: var(--space-1);
		color: var(--muted-fg);
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		font-weight: 400;
	}

	.screens {
		display: flex;
		flex: 1;
		min-height: 0;
		overflow-x: auto;
		overflow-y: hidden;
		overscroll-behavior-x: contain;
		scroll-snap-type: x mandatory;
		scrollbar-width: none;
	}

	.held {
		overflow-x: hidden;
	}

	@media (prefers-reduced-motion: no-preference) {
		.screens {
			scroll-behavior: smooth;
		}
	}

	.screen {
		display: flex;
		flex: none;
		flex-direction: column;
		width: 100%;
		scroll-snap-align: start;
		scroll-snap-stop: always;
	}

	main {
		display: grid;
		flex: 1;
		grid-template-rows: auto minmax(0, 1fr);
		grid-template-columns: minmax(0, 1fr);
		min-width: 0;
		min-height: 0;
	}

	.strip {
		display: flex;
		grid-row: 1;
		align-items: center;
		min-width: 0;
		border-bottom: var(--border-width) solid var(--border);
		background: var(--surface-1);
		user-select: none;
	}

	.divider {
		position: relative;
		z-index: 1;
		grid-row: 1 / -1;
		grid-column: 2;
		background: var(--border);
		cursor: col-resize;
		touch-action: none;
	}

	.divider::before {
		content: '';
		position: absolute;
		inset: 0 calc(-1 * var(--space-1));
	}

	[role='tablist'] {
		display: flex;
		flex: 1;
		align-self: stretch;
		min-width: 0;
		min-height: calc(var(--target-min) + var(--space-2));
	}

	.phone [role='tablist'] {
		overflow-x: auto;
		scrollbar-width: none;
	}

	.tabs {
		display: grid;
		grid-row: 2;
		grid-column: 1 / -1;
		grid-template-rows: minmax(0, 1fr);
		grid-template-columns: subgrid;
	}

	.view {
		grid-row: 1;
		min-width: 0;
	}

	.zones {
		position: relative;
		z-index: 2;
		display: flex;
		grid-row: 1;
		grid-column: 1 / -1;
		gap: var(--space-2);
		padding: var(--space-2);
		pointer-events: none;
	}

	.zone {
		flex-basis: 0;
		border: 2px dashed var(--border);
		border-radius: var(--radius-md);
		transition:
			background var(--dur-fast),
			border-color var(--dur-fast);
	}

	.zone.over {
		border-color: var(--color-fg);
		background: color-mix(in oklab, var(--color-fg) 8%, transparent);
	}

	.empty {
		grid-row: 1;
		grid-column: 1 / -1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: var(--space-3);
		height: 100%;
		color: var(--muted-fg);
	}

	.empty p {
		margin: 0;
	}

	.reconnecting,
	.update {
		position: fixed;
		top: var(--space-2);
		left: 50%;
		z-index: 55;
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-2) var(--space-3);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
		background: color-mix(in oklab, var(--status-waiting) 16%, var(--color-bg));
		box-shadow: var(--shadow-float);
		translate: -50%;
		max-width: calc(100vw - 2 * var(--space-2));
		box-sizing: border-box;
	}

	.reconnecting {
		white-space: nowrap;
		pointer-events: none;
	}

	.reconnecting :global(svg) {
		color: var(--status-waiting);
	}

	.ended {
		display: flex;
		flex: none;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-3);
		padding: var(--space-2) var(--space-3);
		border-bottom: var(--border-width) solid var(--border);
		background: var(--surface-1);
		color: var(--muted-fg);
	}

	.message {
		display: inline-flex;
		flex: 1 1 16rem;
		align-items: center;
		gap: var(--space-2);
	}

	.message :global(svg) {
		flex: none;
	}

	.choices {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}
</style>
