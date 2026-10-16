<script lang="ts">
	import type { Device } from '#lib/device.svelte.ts';
	import { list, type Entry } from '#lib/files.svelte.ts';
	import { letters, type Change } from '#lib/git.ts';
	import IconButton from '#lib/ui/IconButton.svelte';
	import { onMount } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import CaretRight from '~icons/ph/caret-right';
	import Eye from '~icons/ph/eye';
	import EyeSlash from '~icons/ph/eye-slash';

	type Props = { projectId: string; device: Device; changes: Change[]; onopen(path: string): void };

	let { projectId, device, changes, onopen }: Props = $props();

	let includeIgnored = $state(false);
	let listings = $state<Record<string, Entry[]>>({});
	const expanded = new SvelteSet<string>();
	const kinds = $derived(new Map(changes.map((change) => [change.path, change.kind])));
	const marked = $derived(new Set(changes.flatMap(({ path, from }) => [...parents(path), ...parents(from ?? '')])));

	onMount(() => {
		load('');
		return device.listen({
			message(message) {
				if (message.type === 'hello') return reload();
				if (message.type !== 'fs.changed' || message.projectId !== projectId) return;
				const dirs = new Set(message.paths.map((path) => path.slice(0, Math.max(path.lastIndexOf('/'), 0))));
				for (const dir of dirs) if (dir === '' || expanded.has(dir)) load(dir);
			}
		});
	});

	function parents(path: string) {
		const parts = path.split('/');
		return parts.slice(1).map((_, index) => parts.slice(0, index + 1).join('/'));
	}

	async function load(path: string) {
		listings[path] = await list(projectId, path, includeIgnored);
	}

	function toggle(path: string) {
		if (expanded.delete(path)) return;
		expanded.add(path);
		load(path);
	}

	function reload() {
		for (const path of ['', ...expanded]) load(path);
	}

	function showIgnored() {
		includeIgnored = !includeIgnored;
		reload();
	}
</script>

{#snippet tree(path: string, depth: number)}
	<ul>
		{#each listings[path] ?? [] as entry (entry.path)}
			{@const open = entry.dir && expanded.has(entry.path)}
			{@const kind = entry.dir ? undefined : kinds.get(entry.path)}
			<li>
				<button
					class:ignored={entry.ignored}
					class:changed={kind}
					style:--depth={depth}
					style:--git={kind && `var(--git-${kind})`}
					aria-expanded={entry.dir ? open : undefined}
					title={kind ? `${entry.path} (${kind})` : entry.path}
					onclick={() => (entry.dir ? toggle(entry.path) : onopen(entry.path))}
				>
					<span class="caret" class:open>{#if entry.dir}<CaretRight />{/if}</span>
					<span class="name">{entry.name}</span>
					{#if kind}
						<span class="mark" aria-hidden="true">{letters[kind]}</span>
					{:else if entry.dir && marked.has(entry.path)}
						<span class="mark" aria-hidden="true">•</span>
					{/if}
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
		flex: 1;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.changed .name {
		color: var(--git);
	}

	.mark {
		flex: none;
		color: var(--git, var(--muted-fg));
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		font-weight: 700;
	}
</style>
