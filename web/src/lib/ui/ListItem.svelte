<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { HTMLAttributes } from 'svelte/elements';

	type Props = Omit<HTMLAttributes<HTMLElement>, 'class'> & { element?: 'div' | 'button'; size?: 'md'; highlighted?: boolean; disabled?: boolean; children: Snippet };

	let { element = 'div', size, highlighted, children, ...rest }: Props = $props();
</script>

<!-- Lists that track their own highlight (bits roles, or `highlighted`) skip :hover so the pointer can't light a second row. -->
<svelte:element
	this={element}
	type={element === 'button' ? 'button' : undefined}
	class={['item', size]}
	class:hoverable={highlighted === undefined && !rest.role}
	data-highlighted={highlighted || undefined}
	{...rest}
>
	{@render children()}
</svelte:element>

<style>
	.item {
		position: relative;
		display: flex;
		box-sizing: border-box;
		flex: none;
		align-items: center;
		gap: var(--space-2);
		width: 100%;
		min-width: 0;
		min-height: var(--target-min);
		padding: var(--space-1) var(--space-2);
		border: none;
		border-radius: var(--radius-md);
		background: none;
		color: inherit;
		font: inherit;
		text-align: start;
		cursor: pointer;
	}

	.item :global(svg) {
		flex: none;
	}

	.md {
		gap: var(--space-3);
		min-height: calc(var(--target-min) + var(--space-2));
		padding-inline: var(--space-3);
	}

	.item[data-highlighted],
	.item[data-selected],
	.hoverable:hover:not(:disabled) {
		background: var(--hover);
	}

	.item[aria-current]:not([aria-current='false']) {
		background: var(--color-bg);
		box-shadow:
			var(--shadow-lift),
			0 0 0 var(--border-width) var(--border);
		color: var(--color-fg);
		font-weight: 600;
	}

	.item[data-disabled],
	.item:disabled {
		color: var(--muted-fg);
		cursor: default;
	}

	.item:focus-visible {
		outline: var(--focus-ring);
		outline-offset: var(--focus-inset);
	}

	.item[data-highlighted]:focus-visible {
		outline: none;
	}
</style>
