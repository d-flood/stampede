<script lang="ts">
	import { clips, type Clip } from '#lib/config.svelte.ts';
	import ListItem from '#lib/ui/ListItem.svelte';
	import Suggestions from '#lib/ui/Suggestions.svelte';

	type Props = {
		text: string;
		caret: number;
		replace(text: string, caret: number): void;
	};

	let { text, caret, replace }: Props = $props();

	const query = $derived(text.slice(0, caret).match(/(?:^|\s);(\S*)$/)?.[1]);
	const matches = $derived(query === undefined ? [] : clips().filter((clip) => clip.name.toLowerCase().startsWith(query.toLowerCase())));
	let highlighted = $state<string>();
	const highlight = $derived(Math.max(0, matches.findIndex((clip) => clip.name === highlighted)));
	let dismissed = $state<string>();
	const open = $derived(matches.length > 0 && dismissed !== text);

	function choose(clip: Clip) {
		const before = text.slice(0, caret - query!.length - 1);
		const after = text.slice(caret).replace(/^\S*/, '');
		replace(before + clip.text + after, before.length + clip.text.length);
	}

	export function key(event: KeyboardEvent) {
		if (!open) return;
		if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
			highlighted = matches[(highlight + (event.key === 'ArrowDown' ? 1 : matches.length - 1)) % matches.length].name;
		} else if ((event.key === 'Enter' || event.key === 'Tab') && !event.shiftKey) {
			choose(matches[highlight]);
		} else if (event.key === 'Escape') {
			dismissed = text;
		} else {
			return;
		}
		event.preventDefault();
	}
</script>

{#if open}
	<Suggestions label="Clips">
		{#each matches as clip, index (clip.name)}
			<ListItem
				element="button"
				aria-label={`;${clip.name}`}
				title={clip.text}
				highlighted={index === highlight}
				onpointerenter={() => (highlighted = clip.name)}
				onmousedown={(event) => event.preventDefault()}
				onclick={() => choose(clip)}
			>
				<span class="name">;{clip.name}</span><span class="text">{clip.text}</span>
			</ListItem>
		{/each}
	</Suggestions>
{/if}

<style>

	.name,
	.text {
		font-family: var(--font-mono);
		font-size: var(--font-size-md);
		white-space: nowrap;
	}

	.name {
		flex: none;
	}

	.text {
		overflow: hidden;
		color: var(--muted-fg);
		text-overflow: ellipsis;
	}
</style>
