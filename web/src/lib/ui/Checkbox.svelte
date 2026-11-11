<script lang="ts">
	import type { HTMLInputAttributes } from 'svelte/elements';

	type Props = Omit<HTMLInputAttributes, 'type'> & { type?: 'checkbox' | 'radio'; tone?: 'accent' | 'waiting' };

	let { type = 'checkbox', tone = 'accent', checked = $bindable(), ...rest }: Props = $props();
</script>

{#if type === 'radio'}
	<input type="radio" class={tone} {checked} {...rest} />
{:else}
	<input type="checkbox" class={tone} bind:checked {...rest} />
{/if}

<style>
	input {
		--check: var(--accent);
		appearance: none;
		display: inline-grid;
		place-content: center;
		flex: none;
		box-sizing: border-box;
		width: 1.15em;
		height: 1.15em;
		margin: 0;
		border: calc(1.5 * var(--border-width)) solid var(--muted-fg);
		border-radius: calc(var(--radius-sm) / 2);
		background: none;
		cursor: pointer;
		transition:
			border-color var(--dur-fast),
			background var(--dur-fast);
	}

	.waiting {
		--check: var(--status-waiting);
	}

	input[type='radio'] {
		border-radius: 50%;
	}

	input:checked {
		border-color: var(--check);
	}

	input[type='radio']:checked {
		background: radial-gradient(circle, var(--check) 42%, transparent 48%);
	}

	input[type='checkbox']:checked {
		background: var(--check);
	}

	input[type='checkbox']:checked::before {
		content: '';
		width: 0.7em;
		height: 0.7em;
		background: var(--color-bg);
		clip-path: polygon(14% 44%, 0 65%, 40% 100%, 100% 16%, 82% 0, 38% 65%);
	}

	input:focus-visible {
		outline: var(--focus-ring);
		outline-offset: var(--focus-offset);
	}

	input:disabled {
		opacity: 0.5;
		cursor: default;
	}
</style>
