<script lang="ts">
	import { clips, type Clip } from '#lib/config.svelte.ts';

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
	<section aria-label="Clips" class="menu">
		{#each matches as clip, index (clip.name)}
			<button
				class="row"
				aria-label={`;${clip.name}`}
				title={clip.text}
				class:highlighted={index === highlight}
				onpointerenter={() => (highlighted = clip.name)}
				onmousedown={(event) => event.preventDefault()}
				onclick={() => choose(clip)}
			>
				<span class="name">;{clip.name}</span><span class="text">{clip.text}</span>
			</button>
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
		gap: var(--space-3);
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

	.name {
		flex: none;
	}

	.text {
		overflow: hidden;
		color: var(--muted-fg);
		text-overflow: ellipsis;
	}
</style>
