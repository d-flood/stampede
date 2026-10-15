<script lang="ts">
	import { connect, type Device, type Project, type ServerMessage } from '#lib/device.ts';
	import { form } from '#lib/form.ts';
	import { labels } from '#lib/projects.ts';
	import Picker from '#lib/shell/Picker.svelte';
	import Rail from '#lib/shell/Rail.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import { onMount } from 'svelte';
	import SidebarSimple from '~icons/ph/sidebar-simple';

	let device: Device | undefined;
	let projects = $state<Project[]>([]);
	let chosen = $state<string>();
	let sidebar = $state(form() === 'desktop');
	let picking = $state(false);
	let directories = $state<string[]>([]);
	let settle: ((error?: string) => void) | undefined;

	const projectLabels = $derived(labels(projects));
	const focused = $derived(projects.find((project) => project.id === chosen) ?? projects[0]);

	onMount(() => {
		device = connect({ message, output() {} });
		return () => device?.close();
	});

	function message(message: ServerMessage) {
		switch (message.type) {
			case 'hello':
				device?.send({ type: 'projects.list' });
				break;
			case 'projects.changed':
				projects = message.projects;
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
		device?.send({ type: 'picker.list' });
	}

	function open(path: string): Promise<string | undefined> {
		settle?.();
		device?.send({ type: 'project.open', path });
		return new Promise((resolve) => (settle = resolve));
	}

	function reorder(order: string[]) {
		projects = order.flatMap((id) => projects.filter((project) => project.id === id));
		device?.send({ type: 'project.reorder', order });
	}
</script>

<div class="shell">
	<Rail
		{projects}
		labels={projectLabels}
		focused={focused?.id}
		onfocus={(id) => (chosen = id)}
		onreorder={reorder}
		onclose={(projectId) => device?.send({ type: 'project.close', projectId })}
		onpick={pick}
	/>

	{#if sidebar}
		<aside aria-label="Sidebar">
			{#if focused}
				<h1 title={focused.path}>{projectLabels.get(focused.id)}</h1>
			{/if}
		</aside>
	{/if}

	<main>
		<div class="strip">
			<IconButton label="Sidebar" aria-expanded={sidebar} onclick={() => (sidebar = !sidebar)}>
				<SidebarSimple />
			</IconButton>
			<div role="tablist" aria-label="Tabs"></div>
		</div>
		<div class="tabs">
			{#if !focused}
				<div class="empty">
					<p>No Project open</p>
					<Button variant="primary" onclick={pick}>Open Project</Button>
				</div>
			{/if}
		</div>
	</main>
</div>

<Dialog bind:open={picking} title="Open Project">
	<Picker {directories} onopen={open} />
</Dialog>

<style>
	.shell {
		display: flex;
		height: 100dvh;
	}

	aside {
		flex: none;
		width: 20rem;
		border-right: var(--border-width) solid var(--border);
		background: var(--surface-1);
	}

	h1 {
		margin: 0;
		padding: var(--space-3) var(--space-4) 0;
		overflow: hidden;
		font-size: var(--font-size-lg);
		text-overflow: ellipsis;
		white-space: nowrap;
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
