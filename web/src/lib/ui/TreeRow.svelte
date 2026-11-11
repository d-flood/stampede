<script lang="ts">
	import type { Kind } from '#lib/git.ts';
	import Caret from '#lib/ui/Caret.svelte';
	import type { Snippet } from 'svelte';
	import type { HTMLButtonAttributes } from 'svelte/elements';

	type Props = HTMLButtonAttributes & { name: string; depth?: number; open?: boolean; git?: Kind; letter?: string; muted?: boolean; faded?: boolean; children?: Snippet };

	let { name, depth = 0, open, git, letter, muted = false, faded = false, children, ...rest }: Props = $props();
</script>

<button
	type="button"
	class="entry"
	class:muted
	class:deleted={git === 'deleted'}
	style:--depth={depth}
	style:--git={git && `var(--git-${git})`}
	aria-expanded={open}
	{...rest}
>
	{#if open === undefined}<span class="spacer"></span>{:else}<Caret />{/if}
	<span class="name" class:faded>{name}</span>
	{@render children?.()}
	{#if letter}<span class="letter" aria-hidden="true">{letter}</span>{/if}
</button>

<style>
	.entry {
		display: flex;
		flex: 1;
		align-items: center;
		gap: var(--space-1);
		min-width: 0;
		min-height: var(--target-min);
		padding: 0 var(--space-2) 0 calc(var(--space-2) + var(--depth) * var(--space-4));
		border: none;
		border-radius: inherit;
		background: none;
		color: var(--color-fg);
		font: inherit;
		text-align: start;
		cursor: pointer;
	}

	.entry:focus-visible {
		outline: var(--focus-ring);
		outline-offset: var(--focus-inset);
	}

	.spacer {
		flex: none;
		width: 1em;
	}

	.muted {
		color: var(--muted-fg);
	}



	.name {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		color: var(--git, currentColor);
		font-family: var(--font-mono);
		font-size: var(--font-size-code);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.deleted .name {
		text-decoration: line-through;
	}

	.faded {
		opacity: 0.55;
	}

	.letter {
		flex: none;
		color: var(--git, var(--muted-fg));
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		font-weight: 700;
	}
</style>
