<script lang="ts">
	import IconButton from '#lib/ui/IconButton.svelte';
	import { prefersReducedMotion } from 'svelte/motion';
	import { fly } from 'svelte/transition';
	import X from '~icons/ph/x';

	let { queued, disabled, oncancel }: { queued: string[]; disabled: boolean; oncancel(index: number): void } = $props();
</script>

{#if queued.length}
	<ol aria-label="Queued">
		{#each queued as text, index (index)}
			<li in:fly|global={{ y: prefersReducedMotion.current ? 0 : 12, duration: 250 }}><span class="remove"><IconButton type="button" label="Cancel queued message" title="Return to composer" {disabled} onclick={() => oncancel(index)}><X /></IconButton></span>{text}</li>
		{/each}
	</ol>
{/if}

<style>
	ol {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		gap: var(--space-2);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	li {
		position: relative;
		min-height: var(--target-min);
		max-width: 85%;
		padding: var(--space-3) var(--space-4);
		padding-right: calc(var(--target-min) + var(--space-2));
		border: var(--border-width) dashed var(--border);
		border-radius: var(--radius-lg);
		color: var(--muted-fg);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}

	li::before {
		content: 'Queued';
		display: block;
		font-size: var(--font-size-sm);
	}

	.remove {
		position: absolute;
		top: 0;
		right: 0;
		display: flex;
	}
</style>
