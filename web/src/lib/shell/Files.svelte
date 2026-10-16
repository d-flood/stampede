<script lang="ts">
	import { list, type Entry } from '#lib/files.svelte.ts';
	import IconButton from '#lib/ui/IconButton.svelte';
	import { onMount } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import CaretRight from '~icons/ph/caret-right';
	import Eye from '~icons/ph/eye';
	import EyeSlash from '~icons/ph/eye-slash';

	let { projectId, onopen }: { projectId: string; onopen(path: string): void } = $props();

	let includeIgnored = $state(false);
	let listings = $state<Record<string, Entry[]>>({});
	const expanded = new SvelteSet<string>();

	onMount(() => {
		load('');
	});

	async function load(path: string) {
		listings[path] = await list(projectId, path, includeIgnored);
	}

	function toggle(path: string) {
		if (expanded.delete(path)) return;
		expanded.add(path);
		load(path);
	}

	function showIgnored() {
		includeIgnored = !includeIgnored;
		for (const path of ['', ...expanded]) load(path);
	}
</script>

{#snippet tree(path: string, depth: number)}
	<ul>
		{#each listings[path] ?? [] as entry (entry.path)}
			{@const open = entry.dir && expanded.has(entry.path)}
			<li>
				<button
					class:ignored={entry.ignored}
					style:--depth={depth}
					aria-expanded={entry.dir ? open : undefined}
					title={entry.path}
					onclick={() => (entry.dir ? toggle(entry.path) : onopen(entry.path))}
				>
					<span class="caret" class:open>{#if entry.dir}<CaretRight />{/if}</span>
					<span class="name">{entry.name}</span>
				</button>
				{#if open}{@render tree(entry.path, depth + 1)}{/if}
			</li>
		{/each}
	</ul>
{/snippet}

<section aria-label="Files">
	<header>
		<h2>Files</h2>
		<IconButton
			label={includeIgnored ? 'Hide ignored files' : 'Show ignored files'}
			aria-pressed={includeIgnored}
			onclick={showIgnored}
		>
			{#if includeIgnored}<Eye />{:else}<EyeSlash />{/if}
		</IconButton>
	</header>
	<div class="tree">{@render tree('', 0)}</div>
</section>

<style>
	section {
		display: flex;
		flex: 1;
		flex-direction: column;
		min-height: 0;
	}

	header {
		display: flex;
		align-items: center;
		padding: 0 var(--space-1) 0 var(--space-3);
	}

	h2 {
		flex: 1;
		margin: 0;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		font-weight: 600;
	}

	.tree {
		flex: 1;
		overflow: auto;
		padding-bottom: var(--space-3);
	}

	ul {
		margin: 0;
		padding: 0;
		list-style: none;
	}

	button {
		display: flex;
		align-items: center;
		gap: var(--space-1);
		width: 100%;
		min-height: var(--target-min);
		padding: 0 var(--space-3) 0 calc(var(--space-2) + var(--depth) * var(--space-4));
		border: none;
		background: none;
		color: var(--color-fg);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}

	button:hover {
		background: var(--hover);
	}

	button:focus-visible {
		outline: var(--border-width) solid var(--status-working);
		outline-offset: calc(-1 * var(--border-width));
	}

	.ignored {
		color: var(--muted-fg);
	}

	.caret {
		display: inline-flex;
		flex: none;
		width: 1em;
		color: var(--muted-fg);
		transition: transform var(--dur-fast);
	}

	.caret.open {
		transform: rotate(90deg);
	}

	.name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
