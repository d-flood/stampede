<script lang="ts">
	import { listing } from '#lib/files.svelte.ts';
	import { split } from '#lib/git.ts';
	import fuzzysort from 'fuzzysort';

	type Props = {
		text: string;
		caret: number;
		projectId: string;
		replace(text: string, caret: number): void;
	};

	let { text, caret, projectId, replace }: Props = $props();

	const query = $derived(text.slice(0, caret).match(/(?:^|\s)@(\S*)$/)?.[1]);
	const files = $derived(listing(projectId));
	const paths = $derived(query && files.snapshot ? fuzzysort.go(query, files.snapshot, { limit: 50, threshold: 0 }).map((result) => result.target) : []);
	let highlighted = $state<string>();
	const highlight = $derived(Math.max(0, paths.indexOf(highlighted ?? '')));
	let dismissed = $state<string>();
	const open = $derived(query !== undefined && dismissed !== text);

	$effect(() => {
		if (open) files.refresh();
	});

	function choose(path: string) {
		const before = text.slice(0, caret - query!.length - 1);
		const after = text.slice(caret).replace(/^\S*/, '');
		const inserted = /^\s/.test(after) ? path : `${path} `;
		replace(before + inserted + after, before.length + inserted.length);
	}

	export function isOpen() {
		return open;
	}

	export function dismiss() {
		dismissed = text;
	}

	export function key(event: KeyboardEvent) {
		if (!open) return;
		if (paths.length && (event.key === 'ArrowDown' || event.key === 'ArrowUp')) {
			highlighted = paths[(highlight + (event.key === 'ArrowDown' ? 1 : paths.length - 1)) % paths.length];
		} else if (paths.length && (event.key === 'Enter' || event.key === 'Tab') && !event.shiftKey) {
			choose(paths[highlight]);
		} else if (event.key === 'Escape') {
			dismiss();
		} else {
			return;
		}
		event.preventDefault();
	}
</script>

{#if open}
	<section aria-label="Files" class="menu">
		{#each paths as path, index (path)}
			{@const { name, dir } = split(path)}
			<button
				class="row"
				aria-label={path}
				class:highlighted={index === highlight}
				onpointerenter={() => (highlighted = path)}
				onmousedown={(event) => event.preventDefault()}
				onclick={() => choose(path)}
			>
				<span class="parent">{dir}</span><span class="name">{name}</span>
			</button>
		{:else}
			<p class="empty">{!query ? 'Type to find a file' : files.snapshot ? 'No matches' : 'Reading files…'}</p>
		{/each}
	</section>
{/if}

<style>
	.menu {
		position: absolute;
		right: var(--space-2);
		bottom: calc(100% + var(--space-1));
		left: var(--space-2);
		z-index: 1;
		display: flex;
		flex-direction: column;
		max-height: min(24rem, 50vh);
		padding: var(--space-1) 0;
		overflow-y: auto;
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--surface-2);
		box-shadow: var(--shadow-float);
	}

	.row {
		display: flex;
		flex: none;
		align-items: center;
		min-height: var(--target-min);
		padding: var(--space-1) var(--space-4);
		border: 0;
		background: transparent;
		color: var(--color-fg);
		font-family: var(--font-mono);
		font-size: var(--font-size-md);
		text-align: left;
		white-space: nowrap;
		cursor: pointer;
	}

	.row.highlighted {
		background: var(--hover);
	}

	.parent {
		overflow: hidden;
		color: var(--muted-fg);
		text-overflow: ellipsis;
	}

	.name {
		flex: none;
	}

	.empty {
		margin: 0;
		padding: var(--space-2) var(--space-4);
		color: var(--muted-fg);
	}
</style>
