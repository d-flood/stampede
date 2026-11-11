<script lang="ts">
	import { listing } from '#lib/files.svelte.ts';
	import { split } from '#lib/git.ts';
	import ListLabel from '#lib/ui/ListLabel.svelte';
	import Loading from '#lib/ui/Loading.svelte';
	import ListItem from '#lib/ui/ListItem.svelte';
	import Suggestions from '#lib/ui/Suggestions.svelte';
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
	<Suggestions label="Files">
		{#each paths as path, index (path)}
			{@const { name, dir } = split(path)}
			<ListItem
				element="button"
				aria-label={path}
				highlighted={index === highlight}
				onpointerenter={() => (highlighted = path)}
				onmousedown={(event) => event.preventDefault()}
				onclick={() => choose(path)}
			>
				<span class="path"><span class="parent">{dir}</span><span class="name">{name}</span></span>
			</ListItem>
		{:else}
			{#if files.snapshot}
				<ListLabel element="p">{query ? 'No matches' : 'Type to find a file'}</ListLabel>
			{:else if files.failed}
				<ListLabel element="p" role="alert">Could not read files</ListLabel>
			{:else}
				<ListLabel><Loading label="Reading files…" /></ListLabel>
			{/if}
		{/each}
	</Suggestions>
{/if}

<style>

	.path {
		display: flex;
		min-width: 0;
		font-family: var(--font-mono);
		font-size: var(--font-size-md);
		white-space: nowrap;
	}

	.parent {
		overflow: hidden;
		color: var(--muted-fg);
		text-overflow: ellipsis;
	}

	.name {
		flex: none;
	}

</style>
