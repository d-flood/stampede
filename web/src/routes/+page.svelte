<script lang="ts">
	import { version } from '$app/env';
	import { OpenBrowser } from '#lib/browser.svelte.ts';
	import { OpenCanvas } from '#lib/canvas.svelte.ts';
	import Browser from '#lib/shell/Browser.svelte';
	import { clock, same, subagents } from '#lib/chat/agents.ts';
	import Html from '#lib/chat/Html.svelte';
	import { setHtmlPreview, type HtmlItem } from '#lib/chat/html.ts';
	import MessageNav from '#lib/chat/MessageNav.svelte';
	import type ChatViewInstance from '#lib/chat/ChatView.svelte';
	import { hostName, rigs, runsOn, type Rig } from '#lib/config.svelte.ts';
	import { previewThemes, themeBar } from '#lib/themes.svelte.ts';
	import {
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
		type Setting,
		type Terminal,
		type CloudSetup
	} from '#lib/device.svelte.ts';
	import { Devices } from '#lib/devices.svelte.ts';
	import { LanguageServers } from '#lib/editor/lsp.svelte.ts';
	import { heldBy, OpenEpic } from '#lib/epics.svelte.ts';
	import { comparing, image, invalidate, listing, OpenConfig, OpenDiff, OpenFile, OpenChanges, RequestError, type Against, type Cursor, type Reviewing, type Selection } from '#lib/files.svelte.ts';
	import { form } from '#lib/form.ts';
	import { deleteWorktree, status, type Change, type Git, type Trunk, type Update } from '#lib/git.ts';
	import type { PrReview } from '#lib/pr-reviews.ts';
	import { hostOf, hostUrl } from '#lib/host.ts';
	import { capture, putLayout, restore, saveLayout, sendClosing } from '#lib/layout.ts';
	import { afterPaint, cached, warm, warmUp } from '#lib/paint.svelte.ts';
	import { closePanel, openPanel, panel as phonePanel } from '#lib/phone.svelte.ts';
	import { labels, worktrees } from '#lib/projects.ts';
	import { sidebarPosition } from '#lib/sidebar.svelte.ts';
	import AttentionList from '#lib/shell/Attention.svelte';
	import ChatList from '#lib/shell/ChatList.svelte';
	import SchemeToggle from '#lib/shell/SchemeToggle.svelte';
	import ThemeBar from '#lib/shell/ThemeBar.svelte';
	import Settings from '#lib/shell/Settings.svelte';
	import CloseTerminal, { running } from '#lib/shell/CloseTerminal.svelte';
	import Stopping, { type Stopped } from '#lib/shell/Stopping.svelte';
	import Drawer from '#lib/shell/Drawer.svelte';
	import Launchpad from '#lib/shell/Launchpad.svelte';
	import NewTab from '#lib/shell/NewTab.svelte';
	import ProjectIcon from '#lib/shell/ProjectIcon.svelte';
	import Leader from '#lib/shell/Leader.svelte';
	import { menuItems, tabItems } from '#lib/shell/new-tab.ts';
	import type { Searching } from '#lib/shell/session-search.svelte.ts';
	import type { Action, Scope } from '#lib/shell/Palette.svelte';
	import Projects from '#lib/shell/Projects.svelte';
	import Rail from '#lib/shell/Rail.svelte';
	import Rename from '#lib/shell/Rename.svelte';
	import Sidebar, { savedSection, type Section } from '#lib/shell/Sidebar.svelte';
	import TerminalView from '#lib/shell/TerminalView.svelte';
	import Usage from '#lib/shell/Usage.svelte';
	import { arrange, neighbor, type Side, type Split } from '#lib/split.ts';
	import { scrolls, settleInto, swipe } from '#lib/swipe.ts';
	import Button from '#lib/ui/Button.svelte';
	import TabMarks from '#lib/ui/TabMarks.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import FloatingBar from '#lib/ui/FloatingBar.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import Sheet from '#lib/ui/Sheet.svelte';
	import Tab from '#lib/ui/Tab.svelte';
	import TooltipGroup from '#lib/ui/TooltipGroup.svelte';
	import { onMount, tick } from 'svelte';
	import { SvelteMap, SvelteSet } from 'svelte/reactivity';
	import Folders from '~icons/ph/folders';
	import Plus from '~icons/ph/plus';
	import Power from '~icons/ph/power';
	import SidebarSimple from '~icons/ph/sidebar-simple';
	import SlidersHorizontal from '~icons/ph/sliders-horizontal';
	import Swatches from '~icons/ph/swatches';
	import TerminalWindow from '~icons/ph/terminal-window';
	import X from '~icons/ph/x';
	import WifiSlash from '~icons/ph/wifi-slash';

	const loadEditor = cached(() => import('#lib/shell/Editor.svelte'));
	afterPaint(loadEditor);
	const loadCanvasView = cached(() => import('#lib/chat/CanvasView.svelte'));
	const loadEpicView = cached(() => import('#lib/epics/EpicView.svelte'));
	const loadChatView = cached(() => import('#lib/chat/ChatView.svelte'));
	const loadChangesTab = cached(() => import('#lib/shell/ChangesTab.svelte'));
	const loadDiff = cached(() => import('#lib/shell/Diff.svelte'));
	const loadImageView = cached(() => import('#lib/shell/ImageView.svelte'));
	const loadProjectIconPicker = cached(() => import('#lib/shell/ProjectIconPicker.svelte'));
	const loadNewWorktree = cached(() => import('#lib/shell/NewWorktree.svelte'));
	const loadPalette = cached(() => import('#lib/shell/Palette.svelte'));
	const loadPicker = cached(() => import('#lib/shell/Picker.svelte'));
	afterPaint(loadPalette);
	afterPaint(loadPicker);
	const restored = restore();
	const devices = new Devices(message);
	const { home } = devices;
	let projects = $state<Project[]>([]);
	let terminals = $state<Record<string, Terminal[]>>({});
	let chats = $state<Record<string, Chat[]>>({});
	let chatItems = $state<Record<string, Item[]>>({});
	let transcripts = $state<Record<string, HTMLElement | null>>({});
	const chatViews: Record<string, ChatViewInstance> = {};
	let htmlPreview = $state<HtmlItem>();
	let htmlDialog = $state<HTMLDialogElement>();
	setHtmlPreview((item) => htmlPreview = item);
	let openSubagents = $state<Record<string, string[]>>({});
	let sessions = $state<Record<string, Session[]>>({});
	let readingSessions = $state<Record<string, boolean>>({});
	/** Whether each Host, by name (the Home Host's is empty), has any Session to search. */
	let searchable = $state<Record<string, boolean>>({});
	let openError = $state<string>();
	let attentionList = $state<Attention[]>([]);
	let usage = $state<Reading[]>([]);
	let codexResets = $state<ResetCredits | null>(null);
	let trunks = $state.raw<Record<string, Trunk>>({});
	let update = $state.raw<Update>();
	let visible = $state(document.visibilityState === 'visible');
	let files = $state<Record<string, OpenFile[]>>(restored.files);
	let browsers = $state<Record<string, OpenBrowser[]>>(restored.browsers);
	let canvases = $state<Record<string, OpenCanvas[]>>(restored.canvases);
	const offered = new Set<string>();
	const saved = new SvelteSet<string>();
	let epicTabs = $state<Record<string, OpenEpic[]>>(restored.epics);
	let epics = $state.raw<Record<string, Epic[]>>({});
	const subscribed = new Set<string>();
	const git = new SvelteMap<string, Git>();
	const staleGit = new SvelteSet<string>();
	let chosen = $state(restored.chosen);
	let chosenTabs = $state<Record<string, string>>(restored.chosenTabs);
	let setups = $state<Record<string, CloudSetup>>({});
	/** A closed worktree's selected Chat, which its parent selects once it lists it. */
	let adopting: { projectId: string; tabId: string } | undefined;
	let splits = $state<Record<string, Split>>(restored.splits);
	let dragging = $state<{ id: string; x: number; y: number; moved: boolean; over?: Side }>();
	let dragged = false;
	let main: HTMLElement;
	let sidebar = $state(restored.sidebar?.open ?? form() === 'desktop');
	let handoff = false;
	let section = $state<Section>(savedSection(restored.sidebar?.section));
	let sidebarView = $state<Sidebar>();
	let rail = $state(restored.rail?.open ?? false);
	let markdownMode = $state(restored.markdownMode);
	let comparisons = $state<Record<string, Selection | Reviewing>>(restored.compare);
	const plainComparisons: Record<string, Selection> = {};
	for (const [projectId, chosen] of Object.entries(restored.compare)) if ('target' in chosen) plainComparisons[projectId] = chosen;
	let prReviews = $state.raw<Record<string, PrReview[]>>({});
	let closing = $state<OpenFile>();
	let closingProject = $state<string>();
	let closingChat = $state<Chat>();
	let closingTerminal = $state<Terminal>();
	let deletion = $state<{ projectId: string; unmerged?: string; error?: string }>();
	let deleting = $state(false);
	const closingTicket = $derived(closingChat && heldBy(epics[closingChat.projectId] ?? [], closingChat.id));
	const closingWork = $derived(closingChat && { working: 'Its Agent is working. ', waiting: 'Its Agent is waiting on you. ', background: '', done: '' }[chatStatus(closingChat)]);
	let picking = $state(false);
	let iconProject = $state<Project>();
	let branching = $state<Project>();
	let creatingWorktree = $state(false);
	let palette = $state<Scope>();
	let tabTarget = $state<string>();
	let afterPalette: (() => void) | undefined;
	let unfocused = $state(false);
	let directories = $state<string[]>([]);
	let listed = $state(false);
	const panel = $derived(phonePanel());
	let pull = $state(0);
	let starting = $state(false);
	let drawer: Drawer;
	let renamer = $state<Rename>();
	let drawerOpen = $state(false);
	let settle: ((opened: { projectId?: string; error?: string }) => void) | undefined;
	let updateAvailable = $state(false);
	const dirty = $derived(Object.values(files).flat().some((file) => file.dirty));
	const servers = new LanguageServers(
		(projectId) => devices.of(projectId),
		(projectId) => projects.find((project) => project.id === projectId)?.path,
		(projectId, path) => openFile(projectId, path)
	);

	const projectLabels = $derived(labels(projects));
	const ownProjects = $derived(projects.filter((project) => !project.worktree));
	const roots = $derived(new Set(projects.filter((project) => git.get(project.id)?.isRoot).map((project) => project.id)));
	const allChats = $derived(Object.values(chats).flat());
	const working = $derived(
		new Set(projects.filter((project) => chats[project.id]?.some((chat) => chatStatus(chat) === 'working')).map((project) => project.worktree?.parent ?? project.id))
	);
	const background = $derived(
		new Set(projects.filter((project) => chats[project.id]?.some((chat) => chatStatus(chat) === 'background')).map((project) => project.worktree?.parent ?? project.id))
	);
	const needs = $derived(attentionList.reduce((map, item) => map.set(item.projectId, (map.get(item.projectId) ?? 0) + 1), new Map<string, number>()));
	const waitingChats = $derived(new Set(attentionList.flatMap((item) => ('chatId' in item ? [item.chatId] : []))));
	const nested = $derived(worktrees(projects, epics, allChats, chatStatus, needs));
	const focused = $derived(projects.find((project) => project.id === chosen) ?? projects[0]);
	const host = $derived(focused?.host);
	const connectableHost = $derived(focused?.cloud?.state === 'gone' ? undefined : host);
	const unavailable = $derived(focused?.cloud?.state === 'unreachable' || focused?.cloud?.state === 'gone');
	let retrying = $state(false);

	async function retryCloud() {
		if (!host) return;
		retrying = true;
		try { await fetch(`/api/cloud-hosts/${encodeURIComponent(host)}/retry`, { method: 'POST' }); }
		finally { retrying = false; }
	}

	function deleteCloudWorktree(host: string) {
		const project = projects.find((project) => project.host === host);
		if (project) removeWorktree(project.id);
	}
	const tabs = $derived((focused && terminals[focused.id]) || []);
	const chatTabs = $derived((focused && chats[focused.id]) || []);
	const fileTabs = $derived((focused && files[focused.id]) || []);
	const browserTabs = $derived((focused && browsers[focused.id]) || []);
	const canvasTabs = $derived((focused && canvases[focused.id]) || []);
	const openEpics = $derived((focused && epicTabs[focused.id]) || []);
	const chatIds = $derived(new Set(chatTabs.map((chat) => chat.id)));
	const tabIds = $derived([
		...chatTabs.map((chat) => chat.id),
		...tabs.map((tab) => tab.id),
		...browserTabs.map((browser) => browser.id),
		...canvasTabs.map((canvas) => canvas.id),
		...openEpics.map((epic) => epic.id),
		...fileTabs.map((file) => file.id)
	]);
	const closingTabs = $derived(new Set([...tabs.filter((tab) => tab.state === 'closing'), ...chatTabs.filter((chat) => chat.closing)].map(({ id }) => id)));
	const activeTab = $derived.by(() => {
		const tabId = chosenTabs[focused?.id ?? ''];
		const open = (id: string) => !closingTabs.has(id);
		// Falling back while the chosen Tab may be one still to be listed would let its stand-in take the choice over.
		if (tabId && focused && !known(focused.id)) return tabIds.find((id) => id === tabId);
		return tabIds.find((id) => id === tabId && open(id)) ?? tabIds.findLast(open);
	});
	const activeDiff = $derived.by(() => {
		const open = fileTabs.find((file) => file.id === activeTab);
		return open instanceof OpenDiff ? open : undefined;
	});
	const whole = $derived({ tabs: tabIds, active: activeTab });
	const layout = $derived(focused && arrange(splits[focused.id], tabIds, activeTab, closingTabs, (id) => chatIds.has(id)));
	const side: Side = $derived(layout && layout.right.active === activeTab ? 'right' : 'left');
	const split = $derived(form() === 'desktop' ? layout : undefined);
	const groups = $derived(split ? [split.left, split.right] : [whole]);
	const phone = $derived(form() === 'phone');
	const foreground = $derived(visible && !(phone && drawerOpen));
	const shownChats = $derived(foreground ? chatTabs.filter((chat) => showing(chat.id)).map((chat) => chat.id) : []);
	const shownEpics = $derived(foreground ? openEpics.filter((epic) => showing(epic.id)).map((epic) => epic.slug) : []);
	const label = $derived(focused && projectLabels.get(focused.id));
	const gitShown = $derived([...(focused ? [focused.id] : []), ...Object.keys(files).filter((projectId) => files[projectId].length && known(projectId))]);
	const tabActions: Action[] = $derived.by(() => {
		const items = tabTarget === 'drawer'
			? [{ label: 'Terminal', icon: TerminalWindow, onselect: () => drawer.newTab() }]
			: tabTarget ? menuItems(newTab(tabTarget)) : [];
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
	const focusedGit = $derived(focused && git.get(focused.id));
	const deviceLayout = $derived(
		capture({
			chosen,
			terminals,
			chats,
			files,
			browsers,
			canvases,
			epics: epicTabs,
			chosenTabs,
			splits,
			sidebar: { open: sidebar, section },
			rail: { open: rail },
			markdownMode,
			compare: comparisons
		})
	);

	$effect(() => {
		if (connectableHost && visible) return devices.show(connectableHost);
	});
	$effect(() => {
		view();
		for (const host of devices.hosts) view(host);
	});
	$effect(() => saveLayout(deviceLayout));
	$effect(() => {
		if (visible) for (const projectId of gitShown) freshen(projectId);
	});

	const query = new URLSearchParams(location.search);
	const [queryProject, queryTab] = [query.get('project'), query.get('tab')];
	if (queryProject && queryTab) attend(queryProject, queryTab);

	onMount(() => {
		const unlisten = home.listen({ message: (heard) => message(heard) });
		const notified = ({ data }: MessageEvent<Attention>) => attend(data.projectId, tabOf(data));
		navigator.serviceWorker?.addEventListener('message', notified);
		return () => {
			unlisten();
			navigator.serviceWorker?.removeEventListener('message', notified);
			devices.close();
		};
	});

	function message(message: ServerMessage, host?: string) {
		servers.message(message, host);
		switch (message.type) {
			case 'hello': {
				const here = projects.filter((project) => project.host === host);
				readingSessions = elsewhere(readingSessions, host);
				const owned: Record<string, Terminal[]> = Object.fromEntries(here.map(({ id }) => [id, []]));
				for (const terminal of message.terminals) {
					if (terminal.owner !== 'drawer') (owned[terminal.owner.project] ??= []).push(terminal);
				}
				terminals = { ...elsewhere(terminals, host), ...owned };
				const chatsOwned: Record<string, Chat[]> = {};
				for (const chat of message.chats) (chatsOwned[chat.projectId] ??= []).push(chat);
				chats = { ...elsewhere(chats, host), ...chatsOwned };
				const others = Object.values(elsewhere(chats, host)).flat();
				for (const canvas of saved) if (!others.some(({ id }) => canvas.startsWith(`${id}/`))) saved.delete(canvas);
				for (const canvas of message.canvases) saved.add(canvas);
				for (const project of here) {
					invalidate(project.id);
					staleGit.add(project.id);
				}
				view(host);
				if (host) break;
				updateAvailable = (message.buildId ?? message.version) !== version;
				subscribed.clear();
				home.send({ type: 'projects.list' });
				break;
			}
			case 'attention.changed':
				attentionList = message.items;
				break;
			case 'cloud.setup':
				setups[message.projectId] = message;
				break;
			case 'usage.changed':
				usage = message.readings;
				codexResets = message.codexResets ?? null;
				break;
			case 'terminals.changed':
				if (message.projectId) {
					const previous = terminals[message.projectId] ?? [];
					const staged = message.terminals.find((terminal) => terminal.pending && !previous.some(({ id }) => id === terminal.id));
					terminals[message.projectId] = message.terminals;
					if (staged) {
						place(message.projectId, staged.id);
						choose(message.projectId, staged.id);
						if (message.projectId === focused?.id) closePanel();
					}
				}
				break;
			case 'terminal.created': {
				const { owner, id } = message.terminal;
				if (owner !== 'drawer') {
					terminals[owner.project] ??= [];
					const owned = terminals[owner.project];
					if (!owned.some((terminal) => terminal.id === id)) owned.push(message.terminal);
					place(owner.project, id);
					choose(owner.project, id);
					if (owner.project === focused?.id) closePanel();
				}
				break;
			}
			case 'chats.changed': {
				const selected = activeTab;
				if (selected && focused?.worktree?.parent === message.projectId && message.chats.some(({ id }) => id === selected)) {
					chosenTabs[message.projectId] = selected;
				}
				if (adopting?.projectId === message.projectId) {
					const { tabId } = adopting;
					if (message.chats.some(({ id }) => id === tabId)) chosenTabs[message.projectId] = tabId;
					adopting = undefined;
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
			case 'chat.background':
				for (const chat of Object.values(chats).flat()) {
					if (chat.id !== message.chatId) continue;
					chat.background = message.background;
					chat.running = message.running;
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
				const elided = message.items.some((item) => item.kind === 'subagent' && item.id === opened?.[0] && item.elided);
				if (opened && !elided && !subagents(message.items).some(({ path }) => same(path, opened))) delete openSubagents[message.chatId];
				break;
			}
			case 'chat.item':
				(chatItems[message.chatId] ??= [])[message.index] = message.item;
				break;
			case 'chat.append': {
				const item = chatItems[message.chatId]?.[message.index];
				if (item?.kind === 'text') item.text += message.text;
				else if (item?.kind === 'command') item.output += message.text;
				break;
			}
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
					if (chat.id === message.chatId) {
						chat.queued = message.queued;
						chat.queueRevision = message.revision;
					}
				}
				break;
			case 'chat.settings':
				for (const chat of Object.values(chats).flat()) {
					if (chat.id === message.chatId) chat.settings = message.settings;
				}
				break;
			case 'chat.created': {
				if (message.opened) focus(message.chat.projectId);
				chats[message.chat.projectId] ??= [];
				const owned = chats[message.chat.projectId];
				if (!owned.some((chat) => chat.id === message.chat.id)) owned.push(message.chat);
				place(message.chat.projectId, message.chat.id);
				choose(message.chat.projectId, message.chat.id);
				if (message.chat.projectId === focused?.id) closePanel();
				if (prompting && prompting.projectId === message.chat.projectId && prompting.rig === message.chat.rig) {
					devices.of(message.chat.projectId).send({ type: 'chat.send', chatId: message.chat.id, text: prompting.text, queue: false, images: [] });
					prompting = undefined;
				}
				break;
			}
			case 'chat.moved': {
				chats[message.chat.projectId] ??= [];
				const owned = chats[message.chat.projectId];
				if (!owned.some((chat) => chat.id === message.chat.id)) owned.push(message.chat);
				chosen = message.chat.projectId;
				choose(message.chat.projectId, message.chat.id);
				closePanel();
				break;
			}
			case 'chat.open.error':
				openError = message.error;
				break;
			case 'sessions.listed':
				sessions[message.projectId] = message.sessions.filter(({ key }) => runsOn(host, key.agent));
				readingSessions[message.projectId] = false;
				searchable[host ?? ''] = message.coverage.total + message.coverage.ended > 0;
				break;
			case 'terminal.title':
				for (const terminal of Object.values(terminals).flat()) {
					if (terminal.id === message.terminalId) terminal.label = message.label;
				}
				break;
			case 'projects.changed': {
				const left = focused;
				const selected = activeTab;
				const opened = message.projects.filter((project) => !projects.some((other) => other.id === project.id));
				devices.list(new Set(message.projects.flatMap(({ host }) => host ?? [])));
				projects = message.projects;
				listed = true;
				if (!projects.length) home.send({ type: 'picker.list' });
				for (const project of opened) {
					staleGit.add(project.id);
					if (project.cloud?.setupTerminal) chosenTabs[project.id] ??= project.cloud.setupTerminal;
				}
				if (left?.worktree && !projects.some((project) => project.id === left.id)) {
					const { parent } = left.worktree;
					if (selected && chats[left.id]?.some(({ id }) => id === selected)) adopting = { projectId: parent, tabId: selected };
					chosen = parent;
				}
				for (const projectId in terminals) {
					if (!hostOf(projectId) || projects.some((project) => project.id === projectId)) continue;
					for (const { id } of chats[projectId] ?? []) {
						delete chatItems[id];
						delete openSubagents[id];
					}
					delete terminals[projectId];
					delete chats[projectId];
				}
				for (const projectId in browsers) {
					if (!projects.some((project) => project.id === projectId)) delete browsers[projectId];
				}
				for (const projectId in canvases) {
					if (!projects.some((project) => project.id === projectId)) delete canvases[projectId];
				}
				for (const projectId in files) {
					if (!projects.some((project) => project.id === projectId)) delete files[projectId];
				}
				for (const projectId of git.keys()) {
					if (!projects.some((project) => project.id === projectId)) git.delete(projectId);
				}
				for (const projectId of staleGit) {
					if (!projects.some((project) => project.id === projectId)) staleGit.delete(projectId);
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
				const kept = Object.entries(epics).filter(([projectId]) => projects.some((project) => project.id === projectId));
				if (kept.length < Object.keys(epics).length) epics = Object.fromEntries(kept);
				const reviewed = Object.entries(prReviews).filter(([projectId]) => projects.some((project) => project.id === projectId));
				if (reviewed.length < Object.keys(prReviews).length) prReviews = Object.fromEntries(reviewed);
				for (const projectId of subscribed) {
					if (!projects.some((project) => project.id === projectId)) subscribed.delete(projectId);
				}
				for (const { id, host } of projects) {
					if (host || subscribed.has(id)) continue;
					subscribed.add(id);
					home.send({ type: 'epics.subscribe', projectId: id });
					home.send({ type: 'pr-reviews.subscribe', projectId: id });
				}
				if (!activeTab) warmUp();
				break;
			}
			case 'git.changed':
				staleGit.add(message.projectId);
				break;
			case 'origin.changed':
				trunks = { ...elsewhere(trunks, host), ...message.trunks };
				if (!host) update = message.update;
				break;
			case 'pr-reviews.changed':
				prReviews = { ...prReviews, [message.projectId]: message.reviews };
				break;
			case 'epics.changed':
				closeArchived(message.projectId, message.epics);
				epics = { ...epics, [message.projectId]: message.epics };
				break;
			case 'epic.changed': {
				const { epic } = message;
				closeArchived(message.projectId, [epic]);
				const listed = epics[message.projectId] ?? [];
				const index = listed.findIndex((other) => other.slug >= epic.slug);
				const updated = listed[index]?.slug === epic.slug ? listed.with(index, epic) : listed.toSpliced(index === -1 ? listed.length : index, 0, epic);
				epics = { ...epics, [message.projectId]: updated };
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
			case 'canvas.changed': {
				saved.add(message.canvas);
				for (const canvas of Object.values(canvases).flat()) {
					if (canvas.canvas === message.canvas) canvas.changed(message.versions);
				}
				const chat = allChats.find((chat) => message.canvas.startsWith(`${chat.id}/`));
				if (message.versions === 1 && chat && showing(chat.id) && !offered.has(message.canvas)) openCanvas(chat.projectId, message.canvas, chat.id, true);
				break;
			}
			case 'picker.listed':
				directories = message.directories;
				break;
			case 'project.opened':
				focus(message.projectId);
				picking = false;
				settle?.({ projectId: message.projectId });
				break;
			case 'project.error':
				settle?.({ error: message.error });
				break;
		}
	}

	/** Whether a Project's Terminals and Chats are listed, which for a Cloud Host's waits for its Device's first hello. */
	function known(projectId: string) {
		return !hostOf(projectId) || !!terminals[projectId];
	}

	/** Whether a hidden Tab may mount early; a Cloud Host Project's waits for focus, since mounting wakes its Cloud Host. */
	function warms(projectId: string) {
		return warm() && (!hostOf(projectId) || projectId === focused?.id);
	}

	function elsewhere<T>(owned: Record<string, T>, host?: string): Record<string, T> {
		return Object.fromEntries(Object.entries(owned).filter(([projectId]) => hostOf(projectId) !== host));
	}

	function view(on?: string) {
		const here = on === host;
		devices.at(on).send({ type: 'device.view', chatIds: here ? shownChats : [], epics: here ? shownEpics : [] });
	}

	function chatStatus(chat: Chat) {
		if (chat.dormant) return 'done';
		if (chat.status !== 'done') return chat.status;
		return subagents(chatItems[chat.id] ?? []).find(({ subagent }) => subagent.status !== 'done')?.subagent.status ?? (chat.background ? 'background' : 'done');
	}

	function attend(projectId: string, tabId: string) {
		if (drawerOpen) drawer.toggle();
		chosen = projectId;
		const [kind, , slug] = tabId.split(':');
		if (kind === 'epic' && slug) openEpic(projectId, slug);
		else choose(projectId, tabId);
		closePanel();
	}

	function focus(projectId: string) {
		chosen = projectId;
		closePanel();
	}

	const pullsPanels = swipe({
		share: 0.25,
		allow: (event) => !panel && !(event.target as Element).closest('main.bottom > .strip') && !scrolls(event),
		drag: (distance) => (pull = focused || distance > 0 ? distance : 0),
		release: async (distance, committed) => {
			if (committed && pull) await openPanel(distance > 0 ? 'projects' : 'sidebar');
			pull = 0;
		}
	});

	const blocksTabStripHistory = swipe({
		share: 1,
		holds: (event, distance) => !scrolls(event, distance),
		drag: () => {},
		release: () => {}
	});

	/** Closes a phone Tab dragged up off the strip, or settles it back when it falls short or asks first. */
	function liftsTabs(list: HTMLElement) {
		let tab: HTMLElement | undefined;
		let id: string | undefined;
		return swipe({
			share: 1,
			vertical: true,
			direction: -1,
			allow: (event) => {
				const button = (event.target as Element).closest<HTMLElement>('[data-tab]');
				id = button?.dataset.tab;
				tab = button?.closest<HTMLElement>('[role="tablist"] > *') ?? undefined;
				return !!tab;
			},
			drag: (distance) => {
				tab!.style.translate = `0 ${distance}px`;
				tab!.style.opacity = String(Math.max(0.4, 1 + distance / (2 * list.clientHeight)));
			},
			release: (distance, committed) => {
				if (committed && closeAny(id!)) return;
				tab!.style.translate = tab!.style.opacity = '';
				settleInto(tab, [{ translate: `0 ${distance}px` }, { translate: '0' }]);
			}
		})(list);
	}

	async function freshen(projectId: string) {
		if (!staleGit.has(projectId)) return;
		staleGit.delete(projectId);
		const loaded = await status(projectId).catch(() => undefined);
		if (loaded && projects.some((project) => project.id === projectId)) git.set(projectId, loaded);
	}

	function pick() {
		directories = [];
		picking = true;
		home.send({ type: 'picker.list' });
	}

	function open(path: string): Promise<{ projectId?: string; error?: string }> {
		settle?.({});
		home.send({ type: 'project.open', path });
		return new Promise((resolve) => (settle = resolve));
	}

	function reorder(order: string[]) {
		projects = [...order.flatMap((id) => projects.filter((project) => project.id === id)), ...projects.filter((project) => project.worktree)];
		home.send({ type: 'project.reorder', order });
	}

	function stopping(projectId: string) {
		const owned = (id: string) => id === projectId || projects.some((project) => project.id === id && !project.host && project.worktree?.parent === projectId);
		return stoppedBy(allChats.filter((chat) => owned(chat.projectId)), Object.entries(terminals).flatMap(([id, open]) => (owned(id) ? open : [])));
	}

	/** What closing these would stop: busy Chats, then commands still running in Terminals. */
	function stoppedBy(chats: Chat[], shells: Terminal[]): Stopped[] {
		return [
			...chats.filter((chat) => chatStatus(chat) !== 'done').map((chat) => ({ kind: 'chat' as const, label: chat.title ?? chat.rig, agent: chat.agent })),
			...shells.filter(running).flatMap((terminal) => terminal.running ?? []).map((label) => ({ kind: 'command' as const, label }))
		];
	}

	function closeProject(projectId: string, confirmed = false) {
		if (!confirmed && stopping(projectId).length) {
			closingProject = projectId;
			return;
		}
		closingProject = undefined;
		home.send({ type: 'project.close', projectId });
	}

	async function removeWorktree(projectId: string, force = false) {
		deleting = true;
		try {
			await deleteWorktree(projectId, force);
			deletion = undefined;
		} catch (cause) {
			const message = cause instanceof Error ? cause.message : String(cause);
			deletion = cause instanceof RequestError && cause.status === 409 ? { projectId, unmerged: message } : { ...deletion, projectId, error: message };
		} finally {
			deleting = false;
		}
	}

	function readSessions(projectId: string) {
		if (readingSessions[projectId]) return;
		readingSessions[projectId] = true;
		devices.of(projectId).send({ type: 'sessions.list', projectId });
	}

	function newTab(projectId: string) {
		return tabItems(rigs(), sessions[projectId] ?? [], {
			launch: (rig, text, settings) => launch(rig, projectId, text, settings),
			terminal: () => devices.of(projectId).send({ type: 'terminal.create', projectId }),
			browser: () => openBrowser(projectId),
			files: () => openPalette('files'),
			reopen: (session) => reopen(projectId, session),
			search: () => (palette = 'sessions')
		}, hostOf(projectId));
	}

	function reopen(projectId: string, { key }: Session) {
		devices.of(projectId).send({ type: 'chat.open', projectId, key });
	}

	function searching(projectId: string): Searching {
		return { device: devices.of(projectId), projectId, focus: projects.find((project) => project.id === projectId)?.worktree?.parent ?? projectId };
	}

	/** Whether a Session a finder found can be resumed: a Cloud Host can't reopen a closed Project from here. */
	function selectable({ project, key }: Session) {
		return runsOn(hostOf(project.id), key.agent) && (!hostOf(project.id) || projects.some((open) => open.id === project.id));
	}

	/** Focuses the Chat driving a Session, or resumes it in its Project, opening that first if it's closed. */
	function resume(session: Session) {
		const chat = session.chatId ? allChats.find((chat) => chat.id === session.chatId) : undefined;
		if (chat) return attend(chat.projectId, chat.id);
		const projectId = session.project.id;
		if (projects.some((project) => project.id === projectId)) {
			chosen = projectId;
			return reopen(projectId, session);
		}
		if (hostOf(projectId)) return;
		open(session.project.path).then((opened) => {
			if (opened.error) openError = opened.error;
			else if (opened.projectId) reopen(opened.projectId, session);
		});
	}

	/** A first message waiting for its Chat, which the server creates before it can take input. */
	let prompting: { projectId: string; rig: string; text: string } | undefined;

	function launch(rig: Rig, projectId = focused?.id, text?: string, settings?: Partial<Record<Setting, string>>) {
		if (!projectId) return;
		prompting = text ? { projectId, rig: rig.name, text } : undefined;
		devices.of(projectId).send({ type: 'chat.create', projectId, rig: rig.name, settings });
	}

	function jump(index: number) {
		const tabId = groups.flatMap((group) => group.tabs)[index];
		if (focused && tabId && !closingTabs.has(tabId)) choose(focused.id, tabId);
	}

	function openPalette(scope: Scope) {
		if (focused) listing(focused.id).refresh();
		if (scope === 'all') for (const project of projects) readSessions(project.id);
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
		return !unavailable && groups.some((group) => group.active === tabId);
	}

	function column(tabId: string) {
		return split?.right.tabs.includes(tabId) ? 3 : 1;
	}

	function place(projectId: string, tabId: string) {
		const split = splits[projectId];
		if (projectId === focused?.id && side === 'right' && !split.left.tabs.includes(tabId) && !split.right.tabs.includes(tabId)) split.right.tabs.push(tabId);
	}

	function move(tabId: string, to: Side) {
		if (!focused) return;
		const source = layout ? layout[to === 'left' ? 'right' : 'left'] : whole;
		if (!source.tabs.includes(tabId)) return;
		const rest = source.tabs.filter((id) => id !== tabId);
		if (rest.length) {
			const kept = { tabs: rest, active: source.active === tabId ? neighbor(source.tabs, tabId, closingTabs) : source.active };
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
		const next = neighbor(group.tabs, tabId, closingTabs) ?? halves.find((other) => other !== group)?.active;
		if (next && tabId === activeTab) choose(projectId, next);
		else if (next) show(projectId, next);
	}

	function terminalLabel(terminal: Terminal) {
		return terminal.id === focused?.cloud?.setupTerminal ? 'Cloud Setup' : terminal.label;
	}

	async function runSetupAgain(projectId: string) {
		const response = await fetch(hostUrl(hostOf(projectId), '/api/cloud-setup/again'), { method: 'POST' });
		if (!response.ok) return;
		const { terminalId }: CloudSetup = await response.json();
		focus(projectId);
		choose(projectId, terminalId);
	}

	function closeTab(projectId: string, terminalId: string, confirmed = false) {
		const terminal = terminals[projectId]?.find((terminal) => terminal.id === terminalId);
		if (!confirmed && terminal && running(terminal)) {
			closingTerminal = terminal;
			return;
		}
		closingTerminal = undefined;
		leave(projectId, terminalId);
		devices.of(projectId).send({ type: 'terminal.close', terminalId });
	}

	function openTabPicker() {
		tabTarget = drawer.ownsFocus() ? 'drawer' : focused?.id;
		if (!tabTarget) return;
		if (tabTarget !== 'drawer') readSessions(tabTarget);
		palette = 'tabs';
	}

	function openTerminal() {
		if (drawer.ownsFocus()) drawer.newTab();
		else if (focused) devices.of(focused.id).send({ type: 'terminal.create', projectId: focused.id });
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
		if (activeTab) closeAny(activeTab);
	}

	/** Closes a Tab, returning false when it asks to confirm first. */
	function closeAny(tabId: string, confirmed = false) {
		if (!focused) return false;
		const file = fileTabs.find((file) => file.id === tabId);
		const browser = browserTabs.find((browser) => browser.id === tabId);
		const canvas = canvasTabs.find((canvas) => canvas.id === tabId);
		const epic = openEpics.find((epic) => epic.id === tabId);
		const chat = chatTabs.find((chat) => chat.id === tabId);
		if (file) closeFile(file, confirmed);
		else if (browser) closeBrowser(browser);
		else if (canvas) closeCanvas(canvas);
		else if (epic) closeEpic(epic);
		else if (chat) closeChat(chat, confirmed);
		else closeTab(focused.id, tabId, confirmed);
		return !(closing || closingChat || closingTerminal);
	}

	function tabMenu(id: string) {
		const terminal = tabs.find((tab) => tab.id === id);
		const file = fileTabs.find((file) => file.id === id);
		const close = { label: terminal && terminal.state !== 'live' ? 'Dismiss' : 'Close', onselect: () => closeAny(id) };
		if (!file || file instanceof OpenDiff || file instanceof OpenConfig || file instanceof OpenChanges) return [close];
		return [close, { label: 'Rename', onselect: () => renamer?.start(file.path, false) }];
	}

	function closeChat(chat: Chat, confirmed = false) {
		if (!confirmed && (chatStatus(chat) !== 'done' || heldBy(epics[chat.projectId] ?? [], chat.id))) {
			closingChat = chat;
			return;
		}
		closingChat = undefined;
		leave(chat.projectId, chat.id);
		devices.of(chat.projectId).send({ type: 'chat.close', chatId: chat.id });
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
		closePanel();
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
		closePanel();
	}

	function closeBrowser(browser: OpenBrowser) {
		leave(browser.projectId, browser.id);
		browsers[browser.projectId] = browsers[browser.projectId].filter((open) => open.id !== browser.id);
	}

	function openCanvas(projectId: string, name: string, beside: string, offer = false) {
		offered.add(name);
		canvases[projectId] ??= [];
		let canvas = canvases[projectId].find((open) => open.canvas === name);
		if (!canvas) {
			canvas = new OpenCanvas(projectId, name);
			canvases[projectId].push(canvas);
			place(projectId, canvas.id);
			if (projectId === focused?.id && form() === 'desktop') {
				move(canvas.id, splits[projectId]?.right.tabs.includes(beside) ? 'left' : 'right');
				choose(projectId, canvas.id);
				return;
			}
		}
		if (offer) return;
		choose(projectId, canvas.id);
		closePanel();
	}

	function closeCanvas(canvas: OpenCanvas) {
		leave(canvas.projectId, canvas.id);
		canvases[canvas.projectId] = canvases[canvas.projectId].filter((open) => open !== canvas);
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
		closePanel();
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

	function openDiff(projectId: string, { path, from }: Change, against: Against, draft?: string, replace = false) {
		const diff = openTab(new OpenDiff(projectId, path, against, from));
		if (!comparing(against)) return;
		diff.path = path;
		diff.from = from;
		diff.draft = replace ? undefined : draft;
		diff.replacing = replace ? draft : undefined;
	}

	/** Shows a Comparison, or without one returns to the last plain Comparison this Device chose. */
	function compare(projectId: string, selection?: Selection | Reviewing) {
		if (selection && 'target' in selection) plainComparisons[projectId] = selection;
		const next = selection ?? plainComparisons[projectId];
		if (next) comparisons[projectId] = next;
		else delete comparisons[projectId];
	}

	function showComparison(projectId: string, selection: Selection) {
		compare(projectId, selection);
		section = 'compare';
		sidebar = true;
		if (phone) openPanel('sidebar');
	}

	async function openSearch() {
		if (!focused) return;
		section = 'files';
		sidebar = true;
		if (phone) await openPanel('sidebar');
		await tick();
		sidebarView?.focusSearch();
	}

	function showDiff(file: OpenFile, disk: string, buffer: string) {
		openTab(new OpenDiff(file.projectId, file.path, 'disk')).snapshot = { disk, buffer };
	}

	function retarget(projectId: string, from: string, to: string) {
		const open = files[projectId] ?? [];
		const ids = new Map<string, string>();
		for (const file of open) {
			if (file instanceof OpenDiff || file instanceof OpenConfig || file instanceof OpenChanges || (file.path !== from && !file.path.startsWith(`${from}/`))) continue;
			const moved = new OpenFile(projectId, to + file.path.slice(from.length));
			if (open.some((other) => other.id === moved.id)) continue;
			ids.set(file.id, moved.id);
			file.path = moved.path;
		}
		const swap = (id: string) => ids.get(id) ?? id;
		if (chosenTabs[projectId]) chosenTabs[projectId] = swap(chosenTabs[projectId]);
		const stored = splits[projectId];
		for (const group of stored ? [stored.left, stored.right] : []) {
			group.tabs = group.tabs.map(swap);
			if (group.active) group.active = swap(group.active);
		}
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

{#snippet attention()}
	<SchemeToggle />
	<Settings {update} ondeletehost={deleteCloudWorktree} />
	<AttentionList items={attentionList} projects={projectLabels} chats={allChats} onselect={(item) => attend(item.projectId, tabOf(item))} />
{/snippet}

{#snippet launchpad(project: Project)}
	{#key project.id}
		<Launchpad
			name={projectLabels.get(project.id) ?? project.path}
			path={project.path}
			git={git.get(project.id)}
			items={newTab(project.id)}
			sessions={sessions[project.id] ?? []}
			reading={!!readingSessions[project.id]}
			search={searching(project.id)}
			searchable={!!searchable[project.host ?? '']}
			{selectable}
			waiting={allChats.filter((chat) => waitingChats.has(chat.id) && projects.find((other) => other.id === chat.projectId)?.worktree?.parent === project.id)}
			labels={projectLabels}
			onread={() => readSessions(project.id)}
			onsession={resume}
			onchat={(chat) => attend(chat.projectId, chat.id)}
		/>
	{/key}
{/snippet}

{#snippet sidebarPanel()}
	<Sidebar
		bind:this={sidebarView}
		open={phone || (sidebar && !!focused)}
		project={focused}
		{label}
		git={focused && git.get(focused.id)}
		trunk={focused && trunks[focused.id]}
		device={devices.at(host)}
		bind:section
		{usage}
		{codexResets}
		epics={focused && epics[focused.id]}
		comparison={focused && comparisons[focused.id]}
		reviews={focused && prReviews[focused.id]}
		{activeDiff}
		oncompare={(selection) => focused && compare(focused.id, selection)}
		onopen={(path, cursor) => focused && openFile(focused.id, path, cursor)}
		onrename={(from, to) => focused && retarget(focused.id, from, to)}
		ondiff={(change, against, draft, replace) => focused && openDiff(focused.id, change, against, draft, replace)}
		onchanges={(projectId) => openTab(new OpenChanges(projectId))}
		onepic={(slug) => focused && openEpic(focused.id, slug)}
		onfocus={focus}
	>
		{#snippet toggle()}{@render sidebarToggle(true)}{/snippet}
	</Sidebar>
{/snippet}

{#snippet sidebarToggle(inside: boolean)}
	<IconButton
		label="Sidebar"
		aria-expanded={sidebar}
		onclick={() => ((handoff = true), (sidebar = !sidebar))}
		{@attach (node) => {
			if (sidebar === inside && handoff) {
				handoff = false;
				node.focus();
			}
		}}
	>
		<SidebarSimple style={sidebarPosition.side === 'right' ? 'transform: scaleX(-1)' : undefined} />
	</IconButton>
{/snippet}

{#snippet workspace()}
	<main
		bind:this={main}
		class:bottom={phone}
		style:grid-template-columns={unavailable ? undefined : split &&
			`minmax(0, ${split.ratio}fr) var(--border-width) minmax(0, ${1 - split.ratio}fr)`}
	>
		{#if unavailable && focused}
			<div class="cloud-unavailable" role="status">
				<h2>{focused.cloud?.state === 'gone' ? 'Cloud Host gone' : 'Cloud Host unreachable'}</h2>
				<p>{focused.cloud?.state === 'gone' ? `This Cloud Host no longer exists. Commits up to ${focused.cloud.tip?.slice(0, 7) ?? 'an unknown commit'} reached the Home Host.` : focused.cloud?.reason}</p>
				<div class="cloud-actions">
					{#if focused.cloud?.state === 'unreachable'}<Button variant="primary" busy={retrying} disabled={retrying} onclick={retryCloud}>Retry</Button>{/if}
					<Button variant="danger" disabled={deleting} onclick={() => removeWorktree(focused!.id)}>Delete worktree</Button>
				</div>
			</div>
		{:else}
		<TooltipGroup>
			{#each focused || phone ? groups : [] as group, index (index)}
				<div
					class="strip"
					class:current={!phone && group.active === activeTab}
					role="presentation"
					style:grid-column={index * 2 + 1}
					onfocusin={() => focused && group.active && choose(focused.id, group.active)}
					onpointerdown={grab}
					{@attach phone && blocksTabStripHistory}
				>
					{#if !phone && !sidebar && focused && sidebarPosition.side === 'left' && index === 0}
						{@render sidebarToggle(false)}
					{/if}
					<div
						role="tablist"
						aria-label={split ? `${index ? 'Right' : 'Left'} Tabs` : 'Tabs'}
						onwheel={(event) => {
							if (Math.abs(event.deltaY) > Math.abs(event.deltaX)) event.currentTarget.scrollLeft += event.deltaY;
						}}
						{@attach phone && liftsTabs}
					>
						{#if focused}
							{@const projectId = focused.id}
							{@const others = group.tabs.findIndex((id) => !chatIds.has(id))}
							{#each group.tabs as id, position (id)}
								{#if position && position === others}<span class="separator" aria-hidden="true"></span>{/if}
								{@const tab = tabs.find((tab) => tab.id === id)}
								{@const chat = chatTabs.find((chat) => chat.id === id)}
								{@const file = fileTabs.find((file) => file.id === id)}
								{@const browser = browserTabs.find((browser) => browser.id === id)}
								{@const canvas = canvasTabs.find((canvas) => canvas.id === id)}
								{@const epic = openEpics.find((epic) => epic.id === id)}
								{#if tab?.state === 'closing'}
									<Tab variant="terminal" label={terminalLabel(tab)} bottom={phone} closing />
								{:else if chat?.closing}
									<Tab variant="agent" agent={chat.agent} label={chat.title ?? chat.rig} bottom={phone} closing />
								{:else if tab}
									<Menu context items={tabMenu(id)}>
										{#snippet trigger(props)}
											<Tab
												{...props}
												data-tab={id}
												bottom={phone}
												variant="terminal"
												ended={tab.state === 'ended'}
												label={terminalLabel(tab)}
												active={id === group.active}
												onclose={() => closeTab(projectId, id)}
												onclick={() => {
													if (!dragged) choose(projectId, id);
												}}
											/>
										{/snippet}
									</Menu>
								{:else if chat}
									<Menu context items={tabMenu(id)}>
										{#snippet trigger(props)}
											<Tab
												{...props}
												data-tab={id}
												bottom={phone}
												variant="agent"
												agent={chat.agent}
												dormant={chat.dormant}
												status={chat.dormant ? undefined : chatStatus(chat)}
												fresh={!chat.title}
												label={chat.title ?? chat.rig}
												tooltip={chat.title && `${chat.title}\n${chat.rig}`}
												active={id === group.active}
												onclose={() => closeChat(chat)}
												onclick={() => {
													if (!dragged) choose(projectId, id);
												}}
											/>
										{/snippet}
									</Menu>
								{:else if browser}
									<Menu context items={tabMenu(id)}>
										{#snippet trigger(props)}
											<Tab {...props} data-tab={id} variant="browser" label={browser.name} bottom={phone} active={id === group.active}
												onclose={() => closeBrowser(browser)} onclick={() => { if (!dragged) choose(projectId, id); }} />
										{/snippet}
									</Menu>
								{:else if canvas}
									<Menu context items={tabMenu(id)}>
										{#snippet trigger(props)}
											<Tab {...props} data-tab={id} variant="canvas" label={canvas.name} bottom={phone} active={id === group.active}
												onclose={() => closeCanvas(canvas)} onclick={() => { if (!dragged) choose(projectId, id); }} />
										{/snippet}
									</Menu>
								{:else if epic}
									<Menu context items={tabMenu(id)}>
										{#snippet trigger(props)}
											<Tab
												{...props}
												data-tab={id}
												bottom={phone}
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
									<Menu context items={tabMenu(id)}>
										{#snippet trigger(props)}
											<Tab
												{...props}
												data-tab={id}
												bottom={phone}
												variant={file instanceof OpenDiff || file instanceof OpenChanges ? 'diff' : 'editor'}
												against={file instanceof OpenDiff ? file.versus : undefined}
												label={file.name}
												tooltip={file instanceof OpenChanges ? 'Uncommitted changes' : file.deleted ? `${file.path} (deleted on disk)` : file.path}
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
					{#if focused && phone}
						<IconButton label="New Tab" onclick={() => (starting = true)}><Plus /></IconButton>
					{:else if focused}
						{@const projectId = focused.id}
						<Menu items={menuItems(newTab(projectId))} resize loading={readingSessions[projectId] ? 'Reading sessions…' : undefined} onopen={() => readSessions(projectId)}>
							{#snippet trigger(props)}
								{#if group.tabs.length}
									<Button {...props} variant="ghost" size="sm" aria-label="New Tab" title="New tab (Ctrl/Cmd+T)"><Plus /></Button>
								{:else}
									<Button {...props} variant="ghost" size="sm" title="New tab (Ctrl/Cmd+T)"><Plus />New Tab</Button>
								{/if}
							{/snippet}
						</Menu>
					{/if}
					<span class="fill"></span>
					{#if !phone && group.active && chatIds.has(group.active) && !openSubagents[group.active]?.length}
						<MessageNav items={chatItems[group.active] ?? []} scroller={transcripts[group.active]} reveal={(item, land) => chatViews[group.active!]?.reveal(item, land)} />
					{/if}
					{#if focused && index === groups.length - 1}
						{@const projectId = focused.id}
						<ChatList
							chats={chatTabs.filter((chat) => !chat.closing)}
							status={chatStatus}
							waiting={waitingChats}
							sessions={sessions[projectId] ?? []}
							reading={!!readingSessions[projectId]}
							search={searching(projectId)}
							labels={projectLabels}
							{selectable}
							onopen={() => readSessions(projectId)}
							onchat={(chat) => choose(projectId, chat.id)}
							onsession={resume}
						/>
					{/if}
					{#if !phone && !sidebar && focused && sidebarPosition.side === 'right' && index === groups.length - 1}
						{@render sidebarToggle(false)}
					{/if}
				</div>
			{/each}
		</TooltipGroup>
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
		{/if}
		<div class="tabs" class:unavailable>
			{#each Object.values(browsers).flat() as browser (browser.id)}
				<div class="view" hidden={!showing(browser.id)} style:grid-column={column(browser.id)} onfocusin={() => choose(browser.projectId, browser.id)}>
					{#if showing(browser.id) || warms(browser.projectId)}<Browser {browser} focused={browser.id === activeTab} />{/if}
				</div>
			{/each}
			{#each Object.values(canvases).flat() as canvas (canvas.id)}
				<div class="view" hidden={!showing(canvas.id)} style:grid-column={column(canvas.id)} onfocusin={() => choose(canvas.projectId, canvas.id)}>
					{#if showing(canvas.id) || warms(canvas.projectId)}
						{#await loadCanvasView() then { default: CanvasView }}
							<CanvasView {canvas} />
						{/await}
					{/if}
				</div>
			{/each}
			{#each Object.values(epicTabs).flat() as epic (epic.id)}
				<div class="view" hidden={!showing(epic.id)} style:grid-column={column(epic.id)} onfocusin={() => choose(epic.projectId, epic.id)}>
					{#if showing(epic.id) || warm()}
						{#await loadEpicView() then { default: EpicView }}
							<EpicView
								open={epic}
								epic={epics[epic.projectId]?.find((listed) => listed.slug === epic.slug)}
								loaded={!!epics[epic.projectId]}
								active={showing(epic.id)}
								device={devices.of(epic.projectId)}
								chats={allChats}
								oncompare={(selection) => showComparison(epic.projectId, selection)}
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
					{@const setup = setups[projectId]?.terminalId === terminal.id ? setups[projectId] : undefined}
					<div
						class="view"
						hidden={!showing(terminal.id)}
						style:grid-column={column(terminal.id)}
						onfocusin={() => choose(projectId, terminal.id)}
					>
						{#if showing(terminal.id) || warm()}
							<TerminalView
								device={devices.of(projectId)}
								terminalId={terminal.id}
								pending={terminal.pending}
								active={showing(terminal.id)}
								displayed={foreground}
								focused={terminal.id === activeTab && !drawerOpen}
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
								{:else if setup?.state === 'running'}
									<div class="ended" role="status">
										<span class="message"><TabMarks status="working" /> Cloud Setup running · {clock(setup.startedAt)}</span>
									</div>
								{:else if setup?.state === 'failed'}
									<div class="ended failed" role="status">
										<span class="message">Cloud Setup failed{setup.status != null ? `, exit ${setup.status}` : ''}. Fix it in this Terminal or in a Chat, then run it again.</span>
										<div class="choices">
											<Button size="sm" onclick={() => runSetupAgain(projectId)}>Run again</Button>
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
						{#await loadChatView() then { default: ChatView }}
							<ChatView
								bind:this={chatViews[chat.id]}
								device={devices.of(chat.projectId)}
								{chat}
								chats={allChats}
								movable={!hostOf(chat.projectId) && !chat.dormant}
								onchat={(target) => { chosen = target.projectId; choose(target.projectId, target.id); closePanel(); }}
								items={chatItems[chat.id] ?? []}
								saved={[...saved].filter((canvas) => canvas.startsWith(`${chat.id}/`))}
								opened={openSubagents[chat.id] ?? []}
								bind:scroller={transcripts[chat.id]}
								onopen={(path) => (openSubagents[chat.id] = path)}
								active={showing(chat.id)}
								focused={chat.id === activeTab}
								project={projectLabels.get(chat.projectId)}
								root={projects.find(({ id }) => id === chat.projectId)?.path}
								branch={git.get(chat.projectId)?.branch}
								changes={git.get(chat.projectId)?.changes.length ?? 0}
								onchanges={() => openTab(new OpenChanges(chat.projectId))}
								{codexResets}
								ondiff={(path) => openTab(new OpenDiff(chat.projectId, path, 'HEAD'))}
								onfile={(path, cursor) => openFile(chat.projectId, path, cursor)}
								oncanvas={(canvas) => openCanvas(chat.projectId, canvas, chat.id)}
							/>
						{/await}
					{/if}
				</div>
			{/each}
			{#each Object.values(files) as open}
				{#each open as file (file)}
					{@const onshowdiff = (disk: string, buffer: string) => showDiff(file, disk, buffer)}
					{@const active = showing(file.id)}
					{@const device = file instanceof OpenConfig ? home : devices.of(file.projectId)}
					<div
						class="view"
						hidden={!active}
						style:grid-column={column(file.id)}
						onfocusin={() => choose(file.projectId, file.id)}
					>
						{#if active || (warm() && known(file.projectId))}
							{#if file instanceof OpenChanges}
								{#await loadChangesTab() then { default: ChangesTab }}
									<ChangesTab tab={file} git={git.get(file.projectId)} {active} onopen={(path) => openFile(file.projectId, path)} />
								{/await}
							{:else if file instanceof OpenDiff}
								{#await loadDiff() then { default: Diff }}
									<Diff diff={file} git={git.get(file.projectId)} reviews={prReviews[file.projectId]} {device} {servers} {active} displayed={active && foreground} focused={file.id === activeTab} {onshowdiff} />
								{/await}
							{:else if image(file.path) && !/\.svg$/i.test(file.path)}
								{#await loadImageView() then { default: ImageView }}
									<ImageView {file} {device} {active} />
								{/await}
							{:else}
								{#await loadEditor() then { default: Editor }}
									<Editor
										{file}
										{device}
										{servers}
										git={git.get(file.projectId)}
										{active}
										displayed={active && foreground}
										focused={file.id === activeTab}
										bind:markdownMode
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
			{#if !unavailable && focused && known(focused.id) && tabIds.every((id) => closingTabs.has(id))}
				{@render launchpad(focused)}
			{:else if !focused && listed && !picking}
				<div class="empty">
					<img class="mark" src="/icons/icon.svg" alt="" />
					<h2>Open a Project</h2>
					<p class="lede">Choose a folder to work in. Its Chats, Terminals and Git stay together as a Project.</p>
					{#await loadPicker() then { default: Picker }}
						<Picker {directories} onopen={(path) => open(path).then(({ error }) => error)} autofocus={!phone} />
					{/await}
				</div>
			{/if}
		</div>
	</main>
{/snippet}

{#if phone}
	<div class="phone">
		<header>
			<IconButton label="Projects" aria-expanded={panel === 'projects'} onclick={() => openPanel('projects')}>
				{#if focused}<span class="mark-tile squircle"><ProjectIcon project={focused} /></span>{:else}<Folders />{/if}
			</IconButton>
			<h1>
				<span class="name">{label ?? hostName() ?? 'Stampede'}</span>
				{#if focusedGit?.branch}<span class="branch">{focusedGit.branch}</span>{/if}
			</h1>
			<AttentionList items={attentionList} projects={projectLabels} chats={allChats} onselect={(item) => attend(item.projectId, tabOf(item))} />
			{#if focused}
				<IconButton label="Sidebar" aria-expanded={panel === 'sidebar'} onclick={() => openPanel('sidebar')}>
					<SidebarSimple style="transform: scaleX(-1)" />
				</IconButton>
			{/if}
		</header>
		<div class="page" {@attach pullsPanels}>
			{@render workspace()}
		</div>
		{#if themeBar.choice}<ThemeBar choice={themeBar.choice} />{/if}
	</div>
	<Sheet label="Projects" side="left" open={panel === 'projects'} pull={Math.max(pull, 0)} onclose={closePanel}>
		{#if panel === 'projects' || pull > 0 || warm()}
			<div class="sheet-header">
				<h2>Projects{#if hostName()}{' '}<span class="host">{hostName()}</span>{/if}</h2>
				<Settings {update} ondeletehost={deleteCloudWorktree} />
				<IconButton label="Drawer" onclick={() => drawer.toggle()}><TerminalWindow /></IconButton>
			</div>
			<Usage readings={usage} {codexResets} />
			<Projects
				projects={ownProjects}
				labels={projectLabels}
				worktrees={nested}
				{epics}
				{roots}
				{working}
				{background}
				focused={focused?.id}
				onfocus={focus}
				onclose={(projectId) => closeProject(projectId)}
				ondelete={(projectId) => removeWorktree(projectId)}
				onsetup={runSetupAgain}
				onworktree={(project) => (branching = project)}
				onmenu={freshen}
				onpick={pick}
				onicon={(project) => (iconProject = project)}
				{needs}
				{trunks}
			/>
		{/if}
	</Sheet>
	{#if focused}
		<Sheet label="Sidebar" side="right" open={panel === 'sidebar'} pull={Math.max(-pull, 0)} onclose={closePanel}>
			{#if panel === 'sidebar' || pull < 0 || warm()}
				<div class="sheet-header">
					<h2>{label}</h2>
					<IconButton label="Close Sidebar" onclick={closePanel}><X /></IconButton>
				</div>
				{@render sidebarPanel()}
			{/if}
		</Sheet>
	{/if}
{:else}
	<div class="shell">
		<Rail
			bind:open={rail}
			projects={ownProjects}
			labels={projectLabels}
			worktrees={nested}
			{epics}
			{roots}
			{working}
			{background}
			focused={focused?.id}
			onfocus={(id) => (chosen = id)}
			onreorder={reorder}
			onclose={(projectId) => closeProject(projectId)}
			ondelete={(projectId) => removeWorktree(projectId)}
			onsetup={runSetupAgain}
			onworktree={(project) => (branching = project)}
			onmenu={freshen}
			onpick={pick}
			onicon={(project) => (iconProject = project)}
			{needs}
			{trunks}
			{attention}
		/>
		{@render sidebarPanel()}
		{@render workspace()}
	</div>
	{#if themeBar.choice}<ThemeBar choice={themeBar.choice} />{/if}
{/if}

<Drawer bind:this={drawer} bind:open={drawerOpen} device={home} />

{#if phone && focused}
	{@const projectId = focused.id}
	<NewTab bind:open={starting} items={newTab(projectId)} reading={!!readingSessions[projectId]} onread={() => readSessions(projectId)} />
{/if}

{#if focused}
	{@const projectId = focused.id}
	<Rename bind:this={renamer} {projectId} onrename={(from, to) => retarget(projectId, from, to)} />
{/if}

{#if htmlPreview}
	<dialog bind:this={htmlDialog} class="html-preview" aria-label={htmlPreview.title} onclose={() => htmlPreview = undefined} {@attach (node) => { node.showModal(); }}>
		<Html item={htmlPreview} full onclose={() => htmlDialog?.close()} />
	</dialog>
{/if}

{#if home.connection === 'reconnecting' || (!unavailable && devices.at(host).connection === 'reconnecting')}
	<FloatingBar edge="top" tone="waiting" passive role="status"><span class="reconnecting"><WifiSlash /> Reconnecting…</span></FloatingBar>
{:else if updateAvailable}
	<FloatingBar edge="top" tone="waiting" role="status">
		<span>{dirty ? 'Save your edits to reload the update.' : 'Stampede updated.'}</span>
		<Button disabled={dirty} onclick={async () => { await putLayout(deviceLayout); location.reload(); }}>Reload</Button>
	</FloatingBar>
{/if}

<Leader
	{host}
	ontab={jump}
	onnew={openTabPicker}
	onterminal={openTerminal}
	onfiles={() => focused && openPalette('files')}
	onsearch={openSearch}
	onclose={closeActiveTab}
	onpalette={() => openPalette('all')}
	onrig={launch}
	onsplit={() => activeTab && move(activeTab, side === 'left' ? 'right' : 'left')}
/>

<Dialog
	bind:open={() => !!palette, (open) => !open && (palette = undefined)}
	bare
	oncloseautofocus={finishTabSelection}
	title={{ all: 'Command Palette', files: 'Open File', sessions: 'Past Sessions', tabs: 'New Tab' }[palette ?? 'all']}
>
	{#if palette}
		{#await loadPalette() then { default: Palette }}
			{#key palette}
				<Palette
					scope={palette}
					{projects}
					labels={projectLabels}
					{focused}
					actions={palette === 'tabs' ? tabActions : [{ label: 'Open config', icon: SlidersHorizontal, run: openConfig }, { label: 'Preview themes', icon: Swatches, run: () => ((afterPalette = previewThemes), (palette = undefined)) }]}
					chats={allChats.filter((chat) => !chat.closing)}
					status={chatStatus}
					{sessions}
					loadingSessions={!!readingSessions[palette === 'tabs' ? tabTarget ?? '' : focused?.id ?? '']}
					search={focused && searching(focused.id)}
					{selectable}
					onproject={(projectId) => ((chosen = projectId), (palette = undefined))}
					onrig={(launcher) => (launch(launcher), (palette = undefined))}
					onfile={(path) => focused && (openFile(focused.id, path), (palette = undefined))}
					onchat={(chat) => ((afterPalette = () => attend(chat.projectId, chat.id)), (palette = undefined))}
					onsession={(session) => ((afterPalette = () => resume(session)), (palette = undefined))}
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
	{#await loadPicker() then { default: Picker }}
		<Picker {directories} onopen={(path) => open(path).then(({ error }) => error)} autofocus={false} />
	{/await}
</Dialog>

<Dialog bind:open={() => !!iconProject, (open) => !open && (iconProject = undefined)} full={phone} title="Change project icon">
	{#if iconProject}
		{#await loadProjectIconPicker() then { default: ProjectIconPicker }}
			<ProjectIconPicker project={iconProject} onclose={() => (iconProject = undefined)} />
		{/await}
	{/if}
</Dialog>

<Dialog
	bind:open={() => !!branching, (open) => !open && (branching = undefined)}
	title="New worktree"
	dismissible={!creatingWorktree}
	description={branching && `A branch of ${projectLabels.get(branching.id)}, checked out in its own worktree beside it.`}
>
	{#if branching}
		{#await loadNewWorktree() then { default: NewWorktree }}
			<NewWorktree
				project={branching}
				current={git.get(branching.id)?.branch}
				bind:creating={creatingWorktree}
				oncreate={(projectId) => ((creatingWorktree = false), (branching = undefined), focus(projectId))}
				oncancel={() => (branching = undefined)}
			/>
		{/await}
	{/if}
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
		: `${closingWork}Closing the Chat stops its Agent and everything it started.`}
>
	<Stopping items={closingChat?.running ?? []} />
	{#snippet actions()}
		<Button onclick={() => (closingChat = undefined)}>Cancel</Button>
		<Button variant="primary" onclick={() => closingChat && closeChat(closingChat, true)}>Close</Button>
	{/snippet}
</Dialog>

<Dialog
	bind:open={() => !!closingProject, (open) => !open && (closingProject = undefined)}
	title="Close {closingProject && projectLabels.get(closingProject)}?"
	description="Closing it stops these and everything they started."
>
	<Stopping items={closingProject ? stopping(closingProject) : []} />
	{#snippet actions()}
		<Button onclick={() => (closingProject = undefined)}>Cancel</Button>
		<Button variant="primary" onclick={() => closingProject && closeProject(closingProject, true)}>Close</Button>
	{/snippet}
</Dialog>

<Dialog bind:open={() => !!openError, (open) => !open && (openError = undefined)} title="Could not open Session" description={openError}>
	{#snippet actions()}
		<Button variant="primary" onclick={() => (openError = undefined)}>OK</Button>
	{/snippet}
</Dialog>

<CloseTerminal bind:terminal={closingTerminal} onclose={(terminal) => terminal.owner !== 'drawer' && closeTab(terminal.owner.project, terminal.id, true)} />

<Dialog
	bind:open={() => !!deletion, (open) => !open && (deletion = undefined)}
	title={deletion?.unmerged ? `Delete ${projectLabels.get(deletion.projectId)}?` : `Could not delete ${deletion && projectLabels.get(deletion.projectId)}`}
	description={deletion?.unmerged}
>
	{#if deletion?.error}<p role="alert">{deletion.error}</p>{/if}
	{#snippet actions()}
		{#if deletion?.unmerged}
			<Button disabled={deleting} onclick={() => (deletion = undefined)}>Cancel</Button>
			<Button variant="danger" disabled={deleting} onclick={() => deletion && removeWorktree(deletion.projectId, true)}>{deleting ? 'Deleting…' : 'Delete'}</Button>
		{:else}
			<Button variant="primary" onclick={() => (deletion = undefined)}>OK</Button>
		{/if}
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
		gap: var(--gutter);
		box-sizing: border-box;
		height: 100dvh;
		padding: var(--gutter) var(--gutter) var(--gutter) 0;
		background: var(--canvas);
	}

	.shell > main {
		border: var(--border-width) solid var(--pane-border);
		border-radius: var(--radius-xl);
		background: var(--color-bg);
		overflow: hidden;
	}

	.phone {
		display: flex;
		flex-direction: column;
		height: 100dvh;
		background: var(--surface-2);
	}

	header {
		display: flex;
		flex: none;
		align-items: center;
		gap: var(--space-1);
		padding: var(--space-1);
	}

	.mark-tile {
		display: grid;
		width: var(--control-min);
		height: var(--control-min);
		border-radius: var(--radius-md);
		font-family: var(--font-ui);
		overflow: hidden;
	}

	h1 {
		display: flex;
		flex: 1;
		flex-direction: column;
		justify-content: center;
		min-width: 0;
		min-height: var(--target-min);
		margin: 0;
		padding-left: var(--space-1);
		font-size: var(--font-size-lg);
		line-height: 1.2;
	}

	.name,
	.branch,
	.host {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.branch,
	.host {
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		font-weight: 400;
	}

	.page {
		display: flex;
		flex: 1;
		flex-direction: column;
		min-height: 0;
		border-top: var(--border-width) solid var(--pane-border);
		border-radius: var(--radius-xl) var(--radius-xl) 0 0;
		background: var(--color-bg);
		overflow: hidden;
	}

	.sheet-header {
		display: flex;
		flex: none;
		align-items: center;
		gap: var(--space-1);
		padding: var(--space-2) var(--space-2) 0 var(--space-4);
	}

	.sheet-header h2 {
		display: flex;
		flex: 1;
		align-items: baseline;
		gap: var(--space-2);
		min-width: 0;
		margin: 0;
		font-size: var(--font-size-lg);
	}

	.tabs.unavailable { display: none; }

	.cloud-unavailable {
		grid-row: 1 / -1;
		align-self: center;
		justify-self: center;
		padding: var(--space-6);
		text-align: center;
		overflow-wrap: anywhere;
		min-width: 0;
	}
	.cloud-unavailable h2 {
		font-size: var(--font-size-xl);
		margin: 0 0 var(--space-3);
	}
	.cloud-unavailable p {
		color: var(--muted-fg);
		white-space: pre-wrap;
	}
	.cloud-actions {
		display: flex;
		flex-wrap: wrap;
		justify-content: center;
		gap: var(--space-2);
		margin-top: var(--space-4);
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
		gap: var(--space-1);
		min-width: 0;
		padding: 0 var(--space-2);
		border-bottom: var(--border-width) solid var(--border);
		background: var(--color-bg);
		user-select: none;
		container-type: inline-size;
	}

	.strip :global(.controls:has(+ .trigger))::after {
		content: '';
		flex: none;
		align-self: center;
		width: var(--border-width);
		height: 1.15em;
		margin-left: var(--space-1);
		background: var(--border);
	}

	main.bottom {
		grid-template-rows: minmax(0, 1fr) auto;
	}

	.bottom .strip {
		grid-row: 2;
		padding-bottom: var(--space-2);
		border-top: var(--border-width) solid var(--border);
		border-bottom: none;
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
		flex: 0 1 auto;
		align-self: stretch;
		align-items: center;
		gap: var(--space-1);
		min-width: 0;
		min-height: calc(var(--target-min) + 2 * var(--space-2));
		overflow-x: auto;
		overflow-y: hidden;
		scrollbar-width: none;
	}

	.fill {
		flex: 1;
	}

	.separator {
		flex: none;
		align-self: center;
		width: var(--border-width);
		height: 1.25em;
		margin-inline: var(--space-1);
		background: var(--border-strong);
	}

	/* Quiets the unfocused Split's active Tab so focus reads without colour noise. */
	main:not(.bottom) .strip:not(.current) :global([role='tab'][aria-selected='true']) {
		--pill: var(--surface-1);
		color: color-mix(in oklab, var(--color-fg) 55%, var(--muted-fg));
		font-weight: 400;
	}

	.bottom [role='tablist'] {
		min-height: 0;
		overscroll-behavior-x: contain;
		touch-action: pan-x;
	}

	@property --fade-start {
		syntax: '<length>';
		inherits: false;
		initial-value: 0px;
	}

	@property --fade-end {
		syntax: '<length>';
		inherits: false;
		initial-value: 0px;
	}

	@supports (animation-timeline: scroll()) {
		[role='tablist'] {
			mask-image: linear-gradient(to right, transparent, #000 var(--fade-start), #000 calc(100% - var(--fade-end)), transparent);
			animation: edges linear both;
			animation-timeline: scroll(self inline);
		}
	}

	@keyframes edges {
		from {
			--fade-end: 1.5rem;
		}
		10%,
		90% {
			--fade-start: 1.5rem;
			--fade-end: 1.5rem;
		}
		to {
			--fade-start: 1.5rem;
		}
	}

	.tabs {
		display: grid;
		grid-row: 2;
		grid-column: 1 / -1;
		grid-template-rows: minmax(0, 1fr);
		grid-template-columns: subgrid;
	}

	.bottom .tabs {
		grid-row: 1;
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
		align-self: center;
		justify-self: center;
		position: relative;
		width: min(36rem, 100%);
		max-height: 100%;
		padding: var(--space-6);
		box-sizing: border-box;
		overflow: hidden auto;
	}

	.empty h2 {
		margin: 0;
		font: 600 calc(2 * var(--font-size-base))/1.2 var(--font-ui);
	}

	.lede {
		margin: var(--space-2) 0 var(--space-6);
		color: var(--muted-fg);
	}

	.mark {
		display: block;
		width: calc(1.5 * var(--tile));
		height: calc(1.5 * var(--tile));
		margin-bottom: var(--space-6);
	}

	.reconnecting {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		white-space: nowrap;
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

	.failed {
		border-left: var(--space-1) solid var(--ansi-1);
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
