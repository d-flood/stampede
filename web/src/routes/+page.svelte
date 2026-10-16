<script lang="ts">
	import { rigs } from '#lib/config.svelte.ts';
	import { connect, type Project, type ServerMessage, type Terminal } from '#lib/device.ts';
	import { OpenFile } from '#lib/files.svelte.ts';
	import { form } from '#lib/form.ts';
	import { labels } from '#lib/projects.ts';
	import Drawer from '#lib/shell/Drawer.svelte';
	import Editor from '#lib/shell/Editor.svelte';
	import Picker from '#lib/shell/Picker.svelte';
	import Rail from '#lib/shell/Rail.svelte';
	import Sidebar, { type Section } from '#lib/shell/Sidebar.svelte';
	import TerminalView from '#lib/shell/TerminalView.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import Tab from '#lib/ui/Tab.svelte';
	import { onMount } from 'svelte';
	import Plus from '~icons/ph/plus';
	import SidebarSimple from '~icons/ph/sidebar-simple';

	const device = connect();
	let projects = $state<Project[]>([]);
	let terminals = $state<Record<string, Terminal[]>>({});
	let files = $state<Record<string, OpenFile[]>>({});
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

	onMount(() => {
		const unlisten = device.listen({ message });
		return () => {
			unlisten();
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
				break;
			}
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
			case 'projects.changed':
				projects = message.projects;
				for (const projectId in files) {
					if (!projects.some((project) => project.id === projectId)) delete files[projectId];
				}
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

	function openFile(projectId: string, path: string) {
		files[projectId] ??= [];
		let file = files[projectId].find((file) => file.path === path);
		if (!file) {
			file = new OpenFile(projectId, path);
			files[projectId].push(file);
		}
		chosenTabs[projectId] = file.id;
		if (form() === 'phone') sidebar = false;
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

<div class="shell">
	<Rail
		{projects}
		labels={projectLabels}
		focused={focused?.id}
		onfocus={(id) => (chosen = id)}
		onreorder={reorder}
		onclose={(projectId) => closeProject(projectId)}
		onpick={pick}
	/>

	<Sidebar
		open={sidebar}
		project={focused}
		label={focused && projectLabels.get(focused.id)}
		bind:section
		onopen={(path) => focused && openFile(focused.id, path)}
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
						<Menu context items={[{ label: 'Close', onselect: () => closeTab(projectId, tab.id) }]}>
							{#snippet trigger(props)}
								<Tab
									{...props}
									variant={tab.agent ? 'agent' : 'terminal'}
									agent={tab.agent}
									status={tab.status}
									label={tab.rig ?? 'Terminal'}
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
									variant="editor"
									label={file.name}
									title={file.path}
									dirty={file.dirty}
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
					<TerminalView {device} terminalId={terminal.id} active={terminal.id === activeTab} />
				{/each}
			{/each}
			{#each Object.values(files) as open}
				{#each open as file (file.id)}
					<Editor {file} active={file.id === activeTab} />
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
</style>
