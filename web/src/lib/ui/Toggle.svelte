<script lang="ts">
	import type { HTMLButtonAttributes } from 'svelte/elements';

	let { checked = $bindable(false), ...rest }: Omit<HTMLButtonAttributes, 'role' | 'onclick'> & { checked?: boolean } = $props();
</script>

<button type="button" role="switch" aria-checked={checked} onclick={() => (checked = !checked)} {...rest}><span class="track"><span class="thumb"></span></span></button>

<style>
	button {
		display: inline-grid;
		flex: none;
		place-items: center;
		min-height: var(--target-min);
		padding: 0;
		border: none;
		background: none;
		cursor: pointer;
	}

	.track {
		display: flex;
		box-sizing: border-box;
		width: calc(1.75 * var(--control-min));
		height: var(--control-min);
		padding: calc(2 * var(--border-width));
		border-radius: var(--radius-pill);
		background: var(--surface-2);
		transition: background var(--dur-fast);
	}

	.thumb {
		aspect-ratio: 1;
		height: 100%;
		border-radius: 50%;
		background: var(--raised);
		box-shadow: var(--shadow-lift);
		transition: translate var(--dur-med) var(--ease-out);
	}

	button:hover:not(:disabled) .track {
		background: var(--surface-3);
	}

	[aria-checked='true'] .track,
	[aria-checked='true']:hover:not(:disabled) .track {
		background: var(--accent);
	}

	[aria-checked='true'] .thumb {
		translate: calc(0.75 * var(--control-min)) 0;
	}

	button:focus-visible {
		outline: none;
	}

	button:focus-visible .track {
		outline: var(--focus-ring);
		outline-offset: var(--focus-offset);
	}

	button:disabled {
		cursor: default;
		opacity: 0.5;
	}
</style>
