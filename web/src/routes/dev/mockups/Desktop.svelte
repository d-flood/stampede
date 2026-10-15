<script lang="ts">
	import Badge from '#lib/ui/Badge.svelte';
	import Button from '#lib/ui/Button.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import BellSimple from '~icons/ph/bell-simple';
	import Columns from '~icons/ph/columns';
	import Plus from '~icons/ph/plus';
	import Rows from '~icons/ph/rows';
	import TerminalWindow from '~icons/ph/terminal-window';
	import AttentionList from './AttentionList.svelte';
	import Diff from './Diff.svelte';
	import Sidebar from './Sidebar.svelte';
	import Terminal from './Terminal.svelte';
	import { projects, split, tabs, type TabComponent } from './data.ts';

	let { Tab }: { Tab: TabComponent } = $props();
</script>

<div class="desktop">
	<nav class="rail" aria-label="Projects">
		{#each projects as project, n (project.name)}
			<button class="project" class:focused={n === 0} aria-label={project.name}>
				{project.monogram}
				{#if project.attention}<span class="count"><Badge tone="waiting">{project.attention}</Badge></span>{/if}
			</button>
		{/each}
		<IconButton label="Open Project"><Plus /></IconButton>
		<span class="fill"></span>
		<span class="bell">
			<IconButton label="Needs attention" data-state="open"><BellSimple /></IconButton>
			<span class="count"><Badge tone="waiting">2</Badge></span>
		</span>
		<IconButton label="Drawer"><TerminalWindow /></IconButton>
	</nav>

	<aside class="pane">
		<div class="project-name">
			<b>{projects[0].name}</b>
			<span>{projects[0].branch}</span>
		</div>
		<Sidebar />
	</aside>

	<main>
		<section class="pane half">
			<div class="strip">
				<div role="tablist">
					{#each tabs as tab, n (tab.label)}
						<Tab {...tab} active={n === 0} />
					{/each}
				</div>
				<IconButton label="New Tab"><Plus /></IconButton>
			</div>
			<Terminal screen="working" />
		</section>

		<section class="pane half">
			<div class="strip">
				<div role="tablist">
					{#each split as tab, n (tab.label)}
						<Tab {...tab} active={n === 0} />
					{/each}
				</div>
				<IconButton label="New Tab"><Plus /></IconButton>
			</div>
			<div class="toolbar">
				<span class="path"><span>web/src/lib/</span>device.ts</span>
				<Button><span class="switch"></span> TypeScript off</Button>
				<span class="toggle" role="group" aria-label="Diff layout">
					<IconButton label="Side by side" aria-pressed="true"><Columns /></IconButton>
					<IconButton label="Single column"><Rows /></IconButton>
				</span>
			</div>
			<Diff />
		</section>
	</main>

	<div class="popover">
		<AttentionList />
	</div>
</div>

<style>
	.desktop {
		--monogram: calc(var(--target-min) + var(--space-2));

		position: relative;
		display: grid;
		grid-template-columns: auto 20rem minmax(0, 1fr);
		gap: var(--pane-gap);
		box-sizing: border-box;
		height: 100dvh;
		padding: var(--pane-gap) var(--pane-gap) var(--pane-gap) 0;
		background: var(--surface-2);
		font-size: var(--font-size-md);
	}

	.pane {
		display: flex;
		flex-direction: column;
		min-width: 0;
		min-height: 0;
		overflow: hidden;
		border-radius: var(--pane-radius);
	}

	.rail {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-2);
		border-right: var(--border-width) solid var(--border);
	}

	.bell,
	.project {
		position: relative;
	}

	.count {
		position: absolute;
		top: calc(-1 * var(--space-1));
		right: calc(-1 * var(--space-1));
	}

	.project {
		width: var(--monogram);
		height: var(--monogram);
		padding: 0;
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-1);
		color: var(--muted-fg);
		font: inherit;
		font-weight: 600;
		cursor: pointer;
	}

	.project.focused {
		border-color: var(--color-fg);
		background: var(--color-bg);
		color: var(--color-fg);
	}

	.project.focused::before {
		content: '';
		position: absolute;
		top: 20%;
		bottom: 20%;
		left: calc(-1 * var(--space-2) - var(--border-width));
		width: 3px;
		border-radius: 0 3px 3px 0;
		background: var(--color-fg);
	}

	.fill {
		flex: 1;
	}

	aside {
		background: var(--surface-1);
		border-right: var(--border-width) solid var(--border);
	}

	.project-name {
		display: flex;
		align-items: baseline;
		gap: var(--space-2);
		padding: var(--space-3) var(--space-4) 0;
		font-size: var(--font-size-lg);
	}

	.project-name span {
		color: var(--muted-fg);
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
	}

	main {
		display: grid;
		grid-template-columns: minmax(0, 1.3fr) minmax(0, 1fr);
		gap: var(--pane-gap);
		min-height: 0;
	}

	.half {
		background: var(--color-bg);
	}

	.half:first-child {
		border-right: var(--border-width) solid var(--border);
	}

	.strip {
		display: flex;
		align-items: stretch;
		border-bottom: var(--border-width) solid var(--border);
		background: var(--surface-1);
	}

	.strip [role='tablist'] {
		display: flex;
		flex: 1;
		min-width: 0;
		overflow: hidden;
	}

	.toolbar {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-1) var(--space-2) var(--space-1) var(--space-4);
		border-bottom: var(--border-width) solid var(--border);
	}

	.path {
		flex: 1;
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
	}

	.path span {
		color: var(--muted-fg);
	}

	.switch {
		width: 1.8em;
		height: 1em;
		border-radius: 0.5em;
		background: var(--surface-3);
	}

	.switch::after {
		content: '';
		display: block;
		width: 1em;
		height: 1em;
		border-radius: 50%;
		background: var(--muted-fg);
		transform: scale(0.7);
	}

	.toggle {
		display: flex;
	}

	.toggle :global([aria-pressed='true']) {
		background: var(--hover);
		color: var(--color-fg);
	}

	.popover {
		position: absolute;
		bottom: calc(var(--target-min) + 2 * var(--space-2));
		left: calc(var(--monogram) + 2 * var(--space-2) + var(--space-1));
		z-index: 10;
	}
</style>
