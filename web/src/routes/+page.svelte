<script lang="ts">
	import { version } from '$app/env';
	import { rigs, type Rig } from '#lib/config.svelte.ts';
	import { connect, tabOf, type Attention, type Chat, type Project, type ServerMessage, type Terminal } from '#lib/device.svelte.ts';
	import { LanguageServers } from '#lib/editor/lsp.svelte.ts';
	import { invalidate, listing, OpenConfig, OpenDiff, OpenFile, type Cursor } from '#lib/files.svelte.ts';
	import { form } from '#lib/form.ts';
	import { status, type Change, type Commit, type Git } from '#lib/git.ts';
	import { capture, putLayout, restore, saveLayout, sendClosing } from '#lib/layout.ts';
	import { afterPaint, warm, warmUp } from '#lib/paint.svelte.ts';
	import { labels } from '#lib/projects.ts';
	import AttentionList from '#lib/shell/Attention.svelte';
	import ThemeSwitcher from '#lib/shell/ThemeSwitcher.svelte';
	import Drawer from '#lib/shell/Drawer.svelte';
	import Leader from '#lib/shell/Leader.svelte';
	import type { Scope } from '#lib/shell/Palette.svelte';
	import Projects from '#lib/shell/Projects.svelte';
	import Rail from '#lib/shell/Rail.svelte';
	import Sidebar, { showing as sectionShowing, type Section } from '#lib/shell/Sidebar.svelte';
	import TerminalView from '#lib/shell/TerminalView.svelte';
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
	import Plus from '~icons/ph/plus';
	import Power from '~icons/ph/power';
	import SidebarSimple from '~icons/ph/sidebar-simple';
	import TerminalWindow from '~icons/ph/terminal-window';
	import WifiSlash from '~icons/ph/wifi-slash';

	const editor = () => import('#lib/shell/Editor.svelte');
	afterPaint(editor);
	const restored = restore();
	const device = connect();
	let projects = $state<Project[]>([]);
	let terminals = $state<Record<string, Terminal[]>>({});
	let chats = $state<Record<string, Chat[]>>({});
	let attentionList = $state<Attention[]>([]);
	let visible = $state(document.visibilityState === 'visible');
	let files = $state<Record<string, OpenFile[]>>(restored.files);
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
	let picking = $state(false);
	let palette = $state<Scope>();
	let unfocused = $state(false);
	let directories = $state<string[]>([]);
	let screen = $state(1);
	let screens: HTMLElement | undefined;
	let held = $state(false);
	let drawer: Drawer;
	let drawerOpen = $state(false);
	let settle: ((error?: string) => void) | undefined;
	const servers = new LanguageServers(
		device,
		(projectId) => projects.find((project) => project.id === projectId)?.path,
		(projectId, path) => openFile(projectId, path)
	);

	const projectLabels = $derived(labels(projects));
	const focused = $derived(projects.find((project) => project.id === chosen) ?? projects[0]);
	const tabs = $derived((focused && terminals[focused.id]) || []);
	const chatTabs = $derived((focused && chats[focused.id]) || []);
	const fileTabs = $derived((focused && files[focused.id]) || []);
	const tabIds = $derived([
		...tabs.map((tab) => tab.id),
		...chatTabs.map((chat) => chat.id),
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
	const shown = $derived(foreground ? tabs.filter((tab) => showing(tab.id)).map((tab) => tab.id) : []);
	const shownChats = $derived(foreground ? chatTabs.filter((chat) => showing(chat.id)).map((chat) => chat.id) : []);
	const label = $derived(focused && projectLabels.get(focused.id));
	const titles = $derived([
		{ title: 'Projects' },
		{ title: label ?? 'Tabs' },
		{ title: sectionShowing(section, focused && git[focused.id]).label, sub: label }
	]);
	const deviceLayout = $derived(
		capture({ chosen, terminals, chats, files, chosenTabs, splits, sidebar: { open: sidebar, section } })
	);

	$effect(() => device.send({ type: 'device.view', terminalIds: shown, chatIds: shownChats }));
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
				if (message.version !== version && sessionStorage.getItem('reloadedFor') !== message.version) {
					sessionStorage.setItem('reloadedFor', message.version);
					putLayout(deviceLayout).then(() => location.reload());
				}
				const owned: Record<string, Terminal[]> = {};
				for (const terminal of message.terminals) {
					if (terminal.owner !== 'drawer') (owned[terminal.owner.project] ??= []).push(terminal);
				}
				terminals = owned;
				const chatsOwned: Record<string, Chat[]> = {};
				for (const chat of message.chats) (chatsOwned[chat.projectId] ??= []).push(chat);
				chats = chatsOwned;
				for (const project of projects) invalidate(project.id);
				device.send({ type: 'projects.list' });
				device.send({ type: 'device.view', terminalIds: shown, chatIds: shownChats });
				break;
			}
			case 'attention.changed':
				attentionList = message.items;
				break;
			case 'terminals.changed':
				if (message.projectId) terminals[message.projectId] = message.terminals;
				break;
			case 'terminal.status':
				for (const owned of Object.values(terminals)) {
					const terminal = owned.find((terminal) => terminal.id === message.terminalId);
					if (terminal) terminal.status = message.status;
				}
				break;
			case 'terminal.created': {
				const { owner, id } = message.terminal;
				if (owner !== 'drawer') {
					place(owner.project, id);
					choose(owner.project, id);
				}
				break;
			}
			case 'chats.changed':
				chats[message.projectId] = message.chats;
				break;
			case 'chat.status':
				for (const chat of Object.values(chats).flat()) {
					if (chat.id === message.chatId) chat.status = message.status;
				}
				break;
			case 'chat.created':
				place(message.chat.projectId, message.chat.id);
				choose(message.chat.projectId, message.chat.id);
				break;
			case 'terminal.title':
				for (const terminal of Object.values(terminals).flat()) {
					if (terminal.id === message.terminalId) terminal.label = message.label;
				}
				break;
			case 'projects.changed':
				projects = message.projects;
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
				for (const project of projects) loadGit(project.id);
				if (!activeTab) warmUp();
				break;
			case 'git.changed':
				loadGit(message.projectId);
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
		choose(projectId, tabId);
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
		const loaded = await status(projectId);
		if (projects.some((project) => project.id === projectId)) git[projectId] = loaded;
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
		projects = order.flatMap((id) => projects.filter((project) => project.id === id));
		device.send({ type: 'project.reorder', order });
	}

	function closeProject(projectId: string, confirmed = false) {
		const agents = [...(terminals[projectId] ?? []), ...(chats[projectId] ?? [])];
		if (!confirmed && agents.some((agent) => agent.status === 'working')) {
			closingProject = projectId;
			return;
		}
		closingProject = undefined;
		device.send({ type: 'project.close', projectId });
	}

	function newTab(projectId: string) {
		const create = (rig?: string) => () => device.send({ type: 'terminal.create', projectId, rig });
		const chat = (rig: string) => () => device.send({ type: 'chat.create', projectId, rig });
		const launchers = rigs().flatMap((rig) => [
			{ label: rig.name, onselect: create(rig.name) },
			{ label: `${rig.name} as Chat`, onselect: chat(rig.name) }
		]);
		const rest = [{ label: 'Terminal', onselect: create() }, { label: 'Open file', onselect: () => openPalette('files') }];
		return launchers.length ? [{ heading: 'Rigs', items: launchers }, ...rest] : rest;
	}

	function launch(rig: Rig) {
		if (focused) device.send({ type: 'terminal.create', projectId: focused.id, rig: rig.name });
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

	function closeChat(chat: Chat, confirmed = false) {
		if (!confirmed && chat.status === 'working') {
			closingChat = chat;
			return;
		}
		closingChat = undefined;
		leave(chat.projectId, chat.id);
		device.send({ type: 'chat.close', chatId: chat.id });
	}

	function resumable(terminal: Terminal) {
		return !!terminal.rig && !!terminal.agentSession;
	}

	function resume(terminalId: string) {
		device.send({ type: 'terminal.resume', terminalId });
	}

	function tabMenu(projectId: string, tab: Terminal) {
		const close = () => closeTab(projectId, tab.id);
		if (tab.state === 'live') return [{ label: 'Close', onselect: close }];
		const resuming = resumable(tab) ? [{ label: 'Resume', onselect: () => resume(tab.id) }] : [];
		return [...resuming, { label: 'Dismiss', onselect: close }];
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
	<ThemeSwitcher />
	<AttentionList items={attentionList} projects={projectLabels} onselect={(item) => attend(item.projectId, tabOf(item))} />
{/snippet}

{#snippet sidebarPanel()}
	<Sidebar
		open={phone || sidebar}
		project={focused}
		{label}
		git={focused && git[focused.id]}
		{device}
		bind:section
		onopen={(path, cursor) => focused && openFile(focused.id, path, cursor)}
		ondiff={(change, commit) => focused && openDiff(focused.id, change, commit)}
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
							{#if tab}
								<Menu context items={tabMenu(projectId, tab)}>
									{#snippet trigger(props)}
										<Tab
											{...props}
											data-tab={id}
											variant={tab.agent ? 'agent' : 'terminal'}
											agent={tab.agent}
											status={tab.status}
											ended={tab.state === 'ended'}
											label={tab.label}
											active={id === group.active}
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
											status={chat.status}
											label={chat.rig}
											active={id === group.active}
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
					<Menu items={newTab(focused.id)}>
						{#snippet trigger(props)}
							<IconButton {...props} label="New Tab"><Plus /></IconButton>
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
							>
								{#if ended}
									<div class="ended" role="status">
										<span class="message"><Power /> Lost to a Host reboot; showing its last output.</span>
										<div class="choices">
											{#if resumable(terminal)}
												<Button variant="primary" onclick={() => resume(terminal.id)}>Resume</Button>
											{/if}
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
							<ChatView {device} {chat} active={showing(chat.id)} />
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
				<Projects
					{projects}
					labels={projectLabels}
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
			{projects}
			labels={projectLabels}
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

{#if device.connection === 'reconnecting'}
	<div class="reconnecting" role="status"><WifiSlash /> Reconnecting…</div>
{/if}

<Leader
	ontab={jump}
	onpalette={() => openPalette('all')}
	onrig={launch}
	onsplit={() => activeTab && move(activeTab, side === 'left' ? 'right' : 'left')}
/>

<Dialog
	bind:open={() => !!palette, (open) => !open && (palette = undefined)}
	title={palette === 'files' ? 'Open File' : 'Command Palette'}
>
	{#if palette}
		{#await import('#lib/shell/Palette.svelte') then { default: Palette }}
			<Palette
				scope={palette}
				{projects}
				labels={projectLabels}
				{focused}
				actions={[{ label: 'Open config', run: openConfig }]}
				onproject={(projectId) => ((chosen = projectId), (palette = undefined))}
				onrig={(rig) => (launch(rig), (palette = undefined))}
				onfile={(path) => focused && (openFile(focused.id, path), (palette = undefined))}
			/>
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
	description="Its Agent is working. Closing the Chat stops it."
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

	.reconnecting {
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
		white-space: nowrap;
		pointer-events: none;
		translate: -50%;
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
