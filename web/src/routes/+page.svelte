<script lang="ts">
	import { rigs } from '#lib/config.svelte.ts';
	import { connect, type Attention, type Project, type ServerMessage, type Terminal } from '#lib/device.ts';
	import { OpenDiff, OpenFile, type Cursor } from '#lib/files.svelte.ts';
	import { form } from '#lib/form.ts';
	import { status, type Change, type Commit, type Git } from '#lib/git.ts';
	import { labels } from '#lib/projects.ts';
	import AttentionList from '#lib/shell/Attention.svelte';
	import Diff from '#lib/shell/Diff.svelte';
	import Drawer from '#lib/shell/Drawer.svelte';
	import Editor from '#lib/shell/Editor.svelte';
	import Picker from '#lib/shell/Picker.svelte';
	import Rail from '#lib/shell/Rail.svelte';
	import Sidebar, { type Section } from '#lib/shell/Sidebar.svelte';
	import TerminalView from '#lib/shell/TerminalView.svelte';
	import Badge from '#lib/ui/Badge.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import Tab from '#lib/ui/Tab.svelte';
	import { onMount } from 'svelte';
	import Plus from '~icons/ph/plus';
	import Power from '~icons/ph/power';
	import SidebarSimple from '~icons/ph/sidebar-simple';

	const device = connect();
	let projects = $state<Project[]>([]);
	let terminals = $state<Record<string, Terminal[]>>({});
	let attentionList = $state<Attention[]>([]);
	let visible = $state(document.visibilityState === 'visible');
	let files = $state<Record<string, OpenFile[]>>({});
	let git = $state<Record<string, Git>>({});
	let chosen = $state<string>();
	let chosenTabs = $state<Record<string, string>>({});
	let sidebar = $state(form() === 'desktop');
	let section = $state<Section>('files');
	let closing = $state<OpenFile>();
	let closingProject = $state<string>();
	let picking = $state(false);
	let directories = $state<string[]>([]);
	let settle: ((error?: string) => void) | undefined;

	const projectLabels = $derived(labels(projects));
	const focused = $derived(projects.find((project) => project.id === chosen) ?? projects[0]);
	const tabs = $derived((focused && terminals[focused.id]) || []);
	const fileTabs = $derived((focused && files[focused.id]) || []);
	const tabIds = $derived([...tabs.map((tab) => tab.id), ...fileTabs.map((file) => file.id)]);
	const activeTab = $derived(tabIds.find((id) => id === chosenTabs[focused?.id ?? '']) ?? tabIds.at(-1));
	const shown = $derived((visible && tabs.find((tab) => tab.id === activeTab)?.id) || null);

	$effect(() => device.send({ type: 'device.view', terminalId: shown }));

	const query = new URLSearchParams(location.search);
	const [queryProject, queryTerminal] = [query.get('project'), query.get('terminal')];
	if (queryProject && queryTerminal) attend({ projectId: queryProject, terminalId: queryTerminal });

	onMount(() => {
		const unlisten = device.listen({ message });
		const notified = ({ data }: MessageEvent<Attention>) => attend(data);
		navigator.serviceWorker?.addEventListener('message', notified);
		return () => {
			unlisten();
			navigator.serviceWorker?.removeEventListener('message', notified);
			device.close();
		};
	});

	function message(message: ServerMessage) {
		switch (message.type) {
			case 'hello': {
				const owned: Record<string, Terminal[]> = {};
				for (const terminal of message.terminals) {
					if (terminal.owner !== 'drawer') (owned[terminal.owner.project] ??= []).push(terminal);
				}
				terminals = owned;
				device.send({ type: 'projects.list' });
				device.send({ type: 'device.view', terminalId: shown });
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
			case 'terminal.created':
				if (message.terminal.owner !== 'drawer') chosenTabs[message.terminal.owner.project] = message.terminal.id;
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
				for (const project of projects) loadGit(project.id);
				break;
			case 'git.changed':
				loadGit(message.projectId);
				break;
			case 'picker.listed':
				directories = message.directories;
				break;
			case 'project.opened':
				chosen = message.projectId;
				picking = false;
				settle?.();
				break;
			case 'project.error':
				settle?.(message.error);
				break;
		}
	}

	function attend({ projectId, terminalId }: Pick<Attention, 'projectId' | 'terminalId'>) {
		chosen = projectId;
		chosenTabs[projectId] = terminalId;
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
		if (!confirmed && terminals[projectId]?.some((terminal) => terminal.status === 'working')) {
			closingProject = projectId;
			return;
		}
		closingProject = undefined;
		device.send({ type: 'project.close', projectId });
	}

	function newTab(projectId: string) {
		const create = (rig?: string) => () => device.send({ type: 'terminal.create', projectId, rig });
		const launchers = rigs().map((rig) => ({ label: rig.name, onselect: create(rig.name) }));
		const terminal = { label: 'Terminal', onselect: create() };
		return launchers.length ? [{ heading: 'Rigs', items: launchers }, terminal] : [terminal];
	}

	function leave(projectId: string, tabId: string) {
		if (tabId !== activeTab) return;
		const index = tabIds.indexOf(tabId);
		const neighbor = tabIds[index + 1] ?? tabIds[index - 1];
		if (neighbor) chosenTabs[projectId] = neighbor;
	}

	function closeTab(projectId: string, terminalId: string) {
		leave(projectId, terminalId);
		device.send({ type: 'terminal.close', terminalId });
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
		if (!existing) open.push(tab);
		chosenTabs[tab.projectId] = tab.id;
		if (form() === 'phone') sidebar = false;
		return existing ?? tab;
	}

	function openFile(projectId: string, path: string, cursor?: Cursor) {
		const file = openTab(new OpenFile(projectId, path));
		if (cursor) file.cursor = cursor;
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

<div class="shell">
	<Rail
		{projects}
		labels={projectLabels}
		focused={focused?.id}
		onfocus={(id) => (chosen = id)}
		onreorder={reorder}
		onclose={(projectId) => closeProject(projectId)}
		onpick={pick}
	>
		{#snippet badge(project)}
			{@const count = attentionList.filter((item) => item.projectId === project.id).length}
			{#if count}<Badge tone="waiting">{count}</Badge>{/if}
		{/snippet}
		{#snippet attention()}
			<AttentionList items={attentionList} projects={projectLabels} onselect={attend} />
		{/snippet}
	</Rail>

	<Sidebar
		open={sidebar}
		project={focused}
		label={focused && projectLabels.get(focused.id)}
		git={focused && git[focused.id]}
		{device}
		bind:section
		onopen={(path, cursor) => focused && openFile(focused.id, path, cursor)}
		ondiff={(change, commit) => focused && openDiff(focused.id, change, commit)}
	/>

	<main>
		<div class="strip">
			<IconButton label="Sidebar" aria-expanded={sidebar} onclick={() => (sidebar = !sidebar)}>
				<SidebarSimple />
			</IconButton>
			<div role="tablist" aria-label="Tabs">
				{#if focused}
					{@const projectId = focused.id}
					{#each tabs as tab (tab.id)}
						<Menu context items={tabMenu(projectId, tab)}>
							{#snippet trigger(props)}
								<Tab
									{...props}
									variant={tab.agent ? 'agent' : 'terminal'}
									agent={tab.agent}
									status={tab.status}
									ended={tab.state === 'ended'}
									label={tab.label}
									active={tab.id === activeTab}
									onclick={() => (chosenTabs[projectId] = tab.id)}
								/>
							{/snippet}
						</Menu>
					{/each}
					{#each fileTabs as file (file.id)}
						<Menu context items={[{ label: 'Close', onselect: () => closeFile(file) }]}>
							{#snippet trigger(props)}
								<Tab
									{...props}
									variant={file instanceof OpenDiff ? 'diff' : 'editor'}
									against={file instanceof OpenDiff ? file.versus : undefined}
									label={file.name}
									title={file.deleted ? `${file.path} (deleted on disk)` : file.path}
									dirty={file.dirty}
									deleted={file.deleted}
									active={file.id === activeTab}
									onclick={() => (chosenTabs[projectId] = file.id)}
								/>
							{/snippet}
						</Menu>
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
		<div class="tabs">
			{#each Object.entries(terminals) as [projectId, owned] (projectId)}
				{#each owned as terminal (terminal.id)}
					{@const ended = terminal.state === 'ended'}
					<TerminalView {device} terminalId={terminal.id} active={terminal.id === activeTab} {ended}>
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
				{/each}
			{/each}
			{#each Object.values(files) as open}
				{#each open as file (file.id)}
					{@const onshowdiff = (disk: string, buffer: string) => showDiff(file, disk, buffer)}
					{#if file instanceof OpenDiff}
						<Diff diff={file} {device} active={file.id === activeTab} {onshowdiff} />
					{:else}
						<Editor {file} {device} git={git[file.projectId]} active={file.id === activeTab} {onshowdiff} />
					{/if}
				{/each}
			{/each}
			{#if !focused}
				<div class="empty">
					<p>No Project open</p>
					<Button variant="primary" onclick={pick}>Open Project</Button>
				</div>
			{/if}
		</div>
	</main>
</div>

<Drawer {device} />

<Dialog bind:open={picking} title="Open Project">
	<Picker {directories} onopen={open} />
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

	main {
		display: flex;
		flex: 1;
		flex-direction: column;
		min-width: 0;
	}

	.strip {
		display: flex;
		align-items: center;
		border-bottom: var(--border-width) solid var(--border);
		background: var(--surface-1);
	}

	[role='tablist'] {
		display: flex;
		flex: 1;
		align-self: stretch;
		min-width: 0;
		min-height: calc(var(--target-min) + var(--space-2));
	}

	.tabs {
		flex: 1;
		min-height: 0;
	}

	.empty {
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
