<script lang="ts" generics="T">
	import type { Component } from 'svelte';
	import type { HTMLAttributes } from 'svelte/elements';

	type Option = { value: T; label: string; icon?: Component; busy?: boolean; disabled?: boolean };

	let { options, value = $bindable(), disabled = false, ...rest }: HTMLAttributes<HTMLDivElement> & { options: Option[]; value: T; disabled?: boolean } = $props();
</script>

<div class="segmented" role="radiogroup" {...rest}>
	{#each options as option (option.value)}
		<button type="button" role="radio" aria-checked={option.value === value} aria-busy={option.busy || undefined} disabled={disabled || option.disabled} onclick={() => (value = option.value)}>
			<span>{#if option.icon}<option.icon aria-hidden="true" />{/if}<span class="label">{option.label}{#if option.busy}<i class="busy" aria-hidden="true"></i>{/if}</span></span>
		</button>
	{/each}
</div>

<style>
	.segmented {
		position: relative;
		isolation: isolate;
		display: flex;
		flex: none;
		flex-wrap: wrap;
		padding: 0 var(--space-1);
	}

	.segmented::before {
		position: absolute;
		z-index: -1;
		inset: calc((var(--target-min) - var(--control-min)) / 2 - var(--space-1)) 0;
		border-radius: calc(var(--radius-md) + var(--space-1));
		background: var(--surface-2);
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

	button > span {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-1);
		height: var(--control-min);
		padding: 0 var(--space-3);
		border-radius: var(--radius-md);
		white-space: nowrap;
	}

	button:hover:enabled {
		color: var(--color-fg);
	}

	button:disabled {
		cursor: default;
		opacity: 0.5;
	}

	.label {
		position: relative;
	}

	.busy {
		position: absolute;
		top: 0;
		left: calc(100% + var(--space-1));
		width: var(--space-2);
		height: var(--space-2);
		border-radius: 50%;
		background: var(--accent);
		animation: breathe 1.4s ease-in-out var(--pulse);
	}

	@keyframes breathe {
		50% {
			opacity: 0.35;
		}
	}

	button:focus-visible {
		outline: none;
	}

	button:focus-visible > span {
		outline: var(--focus-ring);
		outline-offset: var(--focus-inset);
	}

	[aria-checked='true'] {
		color: var(--color-fg);
		font-weight: 600;
	}

	[aria-checked='true'] > span {
		background: var(--raised);
		box-shadow: var(--shadow-lift);
	}
</style>
