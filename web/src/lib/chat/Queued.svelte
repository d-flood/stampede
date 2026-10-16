<script lang="ts">
	let { queued }: { queued: string[] } = $props();
</script>

{#if queued.length}
	<ol aria-label="Queued">
		{#each queued as text, index (index)}
			<li>{text}</li>
		{/each}
	</ol>
{/if}

<style>
	ol {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	li {
		padding: var(--space-3) var(--space-4);
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
</style>
