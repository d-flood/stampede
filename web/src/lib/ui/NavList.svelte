<script lang="ts">
	import { form } from '#lib/form.ts';
	import type { Snippet } from 'svelte';
	import type { HTMLAttributes } from 'svelte/elements';

	type Props = Omit<HTMLAttributes<HTMLElement>, 'class'> & { element?: 'nav' | 'ul' | 'div'; children: Snippet };

	let { element = 'nav', children, ...rest }: Props = $props();
</script>

<svelte:element this={element} class="list" class:grouped={form() === 'phone'} {...rest}>{@render children()}</svelte:element>

<style>
	.list {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.grouped {
		gap: 0;
		overflow: hidden;
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--color-bg);
	}

	.grouped > :global(* + *) {
		border-top: var(--border-width) solid var(--border);
	}

	.grouped :global(.item) {
		border-radius: 0;
	}

	/* A raised ring would cross the dividers, so grouped rows mark the current one with a fill. */
	.grouped :global(.item[aria-current]:not([aria-current='false'])) {
		background: var(--hover);
		box-shadow: none;
	}
</style>
