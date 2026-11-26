<script lang="ts">
	import type { HTMLInputAttributes } from 'svelte/elements';
	import MagnifyingGlass from '~icons/ph/magnifying-glass';

	let { value = $bindable(), ref = $bindable(), mono = false, search = false, ...rest }: HTMLInputAttributes & { ref?: HTMLInputElement; mono?: boolean; search?: boolean } = $props();
</script>

{#snippet field()}<input bind:this={ref} bind:value class:mono class:search {...rest} />{/snippet}

{#if search}
	<span class="searchbox"><MagnifyingGlass aria-hidden="true" />{@render field()}</span>
{:else}
	{@render field()}
{/if}

<style>
	input {
		box-sizing: border-box;
		width: 100%;
		min-width: 0;
		min-height: var(--target-min);
		padding: var(--space-1) var(--space-2);
		border: var(--border-width) solid var(--border-strong);
		border-radius: var(--radius-md);
		background: var(--color-bg);
		color: var(--color-fg);
		font: inherit;
	}

	.mono {
		font-family: var(--font-mono);
	}

	.mono::placeholder {
		font-family: var(--font-ui);
	}

	.searchbox {
		position: relative;
		display: block;
		min-width: 0;
	}

	.searchbox > :global(svg) {
		position: absolute;
		top: 50%;
		left: var(--space-2);
		color: var(--muted-fg);
		pointer-events: none;
		translate: 0 -50%;
	}

	.search {
		padding-left: calc(var(--space-2) + 1.35em);
		border-color: var(--border);
		background: color-mix(in oklab, var(--hover) 55%, transparent);
	}

	input:focus-visible {
		outline: var(--focus-ring);
		outline-offset: var(--focus-offset);
	}

	input[aria-invalid='true'] {
		border-color: var(--ansi-1);
	}
</style>
