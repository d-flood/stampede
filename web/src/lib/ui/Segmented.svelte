<script lang="ts" generics="T">
	import type { HTMLAttributes } from 'svelte/elements';

	let { options, value = $bindable(), ...rest }: HTMLAttributes<HTMLDivElement> & { options: { value: T; label: string }[]; value: T } = $props();
</script>

<div class="segmented" role="radiogroup" {...rest}>
	{#each options as option (option.value)}
		<button type="button" role="radio" aria-checked={option.value === value} onclick={() => (value = option.value)}><span>{option.label}</span></button>
	{/each}
</div>

<style>
	.segmented {
		position: relative;
		isolation: isolate;
		display: flex;
		flex: none;
		padding: 0 2px;
	}

	.segmented::before {
		position: absolute;
		z-index: -1;
		inset: calc((var(--target-min) - var(--control-min)) / 2 - 2px) 0;
		border-radius: calc(var(--radius-sm) + 2px);
		background: var(--surface-2);
		box-shadow: inset 0 0 0 var(--border-width) var(--border);
		content: '';
	}

	button {
		flex: 1;
		min-height: var(--target-min);
		padding: 0;
		border: none;
		background: none;
		color: var(--muted-fg);
		font: inherit;
		font-size: var(--font-size-sm);
		cursor: pointer;
		transition: color var(--dur-fast);
	}

	span {
		display: flex;
		align-items: center;
		justify-content: center;
		height: var(--control-min);
		padding: 0 var(--space-3);
		border-radius: var(--radius-sm);
		white-space: nowrap;
	}

	button:hover {
		color: var(--color-fg);
	}

	button:focus-visible {
		outline: none;
	}

	button:focus-visible span {
		outline: 2px solid color-mix(in oklab, var(--status-working) 70%, transparent);
		outline-offset: -2px;
	}

	[aria-checked='true'] {
		color: var(--color-fg);
		font-weight: 600;
	}

	[aria-checked='true'] span {
		background: var(--color-bg);
		box-shadow:
			var(--shadow-lift),
			0 0 0 var(--border-width) var(--border);
	}
</style>
