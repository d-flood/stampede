<script lang="ts">
	import Badge from '#lib/ui/Badge.svelte';
	import Button from '#lib/ui/Button.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import BellSimple from '~icons/ph/bell-simple';
	import FolderOpen from '~icons/ph/folder-open';
	import Plus from '~icons/ph/plus';
	import TerminalWindow from '~icons/ph/terminal-window';
	import Sidebar from './Sidebar.svelte';
	import Terminal from './Terminal.svelte';
	import { keys, projects, split, tabs, type TabComponent } from './data.ts';

	let { Tab }: { Tab: TabComponent } = $props();

	const strip = [tabs[1], ...tabs.filter((tab) => tab !== tabs[1]), ...split];
</script>

{#snippet bar(title: string, detail?: string)}
	<header class="bar">
		<h1>{title}{#if detail}<span>{detail}</span>{/if}</h1>
		<span class="bell">
			<IconButton label="Needs attention"><BellSimple /></IconButton>
			<span class="count"><Badge tone="waiting">2</Badge></span>
		</span>
		<IconButton label="Drawer"><TerminalWindow /></IconButton>
	</header>
{/snippet}

<div class="phone">
	<section class="screen" aria-label="Projects">
		{@render bar('Projects')}
		<ul class="projects">
			{#each projects as project, n (project.name)}
				<li class:focused={n === 0}>
					<span class="monogram">{project.monogram}</span>
					<span class="about">
						<b>{project.name}</b>
						<span>{project.path} · {project.branch}</span>
						<span>{project.agents === 1 ? '1 Agent' : `${project.agents || 'No'} Agents`}</span>
					</span>
					{#if project.attention}<Badge tone="waiting">{project.attention}</Badge>{/if}
				</li>
			{/each}
		</ul>
		<div class="open">
			<Button><FolderOpen /> Open Project</Button>
		</div>
	</section>

	<section class="screen" aria-label="Tabs">
		{@render bar(projects[0].name, projects[0].branch)}
		<div class="strip">
			<div role="tablist">
				{#each strip as tab, n (tab.label)}
					<Tab {...tab} active={n === 0} />
				{/each}
			</div>
			<IconButton label="New Tab"><Plus /></IconButton>
		</div>
		<div class="terminal"><Terminal screen="waiting" /></div>
		<form class="composer">
			<textarea rows="3" aria-label="Composer"
				>Yes, but keep 1 s as the first retry and cap the backoff at 30 s.</textarea
			>
			<div class="actions">
				<Button type="button">Insert</Button>
				<Button type="button" variant="primary">Send</Button>
			</div>
		</form>
		<div class="keys" role="toolbar" aria-label="Key bar">
			{#each keys as key (key)}
				<button class="key">{key}</button>
			{/each}
		</div>
	</section>

	<section class="screen" aria-label="Sidebar">
		{@render bar('Changes', projects[0].name)}
		<Sidebar />
	</section>
</div>

<style>
	.phone {
		display: grid;
		grid-auto-columns: 100%;
		grid-auto-flow: column;
		height: 100dvh;
		overflow-x: auto;
		scroll-snap-type: x mandatory;
		scrollbar-width: none;
		font-size: var(--font-size-md);
	}

	.screen {
		display: flex;
		flex-direction: column;
		min-width: 0;
		min-height: 0;
		overflow: hidden;
		scroll-snap-align: start;
	}

	.bar {
		display: flex;
		align-items: center;
		gap: var(--space-1);
		padding: 0 var(--space-1) 0 var(--space-4);
		border-bottom: var(--border-width) solid var(--border);
		background: var(--surface-1);
	}

	h1 {
		display: flex;
		flex: 1;
		align-items: baseline;
		gap: var(--space-2);
		min-width: 0;
		margin: 0;
		font-size: var(--font-size-lg);
	}

	h1 span {
		color: var(--muted-fg);
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		font-weight: 400;
	}

	.bell {
		position: relative;
	}

	.count {
		position: absolute;
		top: var(--space-1);
		right: var(--space-1);
	}

	.projects {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		margin: 0;
		padding: var(--space-3);
		list-style: none;
	}

	.projects li {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		padding: var(--space-3);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--surface-1);
	}

	.projects li.focused {
		border-color: var(--color-fg);
	}

	.monogram {
		display: grid;
		flex: none;
		place-items: center;
		width: var(--target-min);
		height: var(--target-min);
		border-radius: var(--radius-md);
		background: var(--surface-3);
		font-weight: 600;
	}

	.about {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
	}

	.about span {
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.open {
		display: grid;
		padding: 0 var(--space-3);
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
		overflow-x: auto;
		scrollbar-width: none;
	}

	.terminal {
		flex: 1;
		min-height: 0;
	}

	.composer {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		margin: 0 var(--space-2) var(--space-2);
		padding: var(--space-2);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--surface-1);
	}

	textarea {
		box-sizing: border-box;
		width: 100%;
		padding: var(--space-2);
		border: none;
		border-radius: var(--radius-md);
		background: var(--color-bg);
		color: var(--color-fg);
		font: inherit;
		resize: none;
	}

	.actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-2);
	}

	.keys {
		display: flex;
		gap: var(--space-1);
		padding: var(--space-1);
		overflow-x: auto;
		border-top: var(--border-width) solid var(--border);
		background: var(--surface-2);
		scrollbar-width: none;
	}

	.key {
		flex: none;
		min-width: var(--target-min);
		min-height: var(--target-min);
		padding: 0 var(--space-2);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-sm);
		background: var(--surface-1);
		color: var(--color-fg);
		font-family: var(--font-mono);
		font-size: var(--font-size-md);
	}
</style>
