<script lang="ts">
	import { render } from '#lib/chat/markdown.ts';

	let { text }: { text: string } = $props();
	const html = $derived(render(text));
</script>

<div class="preview" role="region" aria-label="Markdown preview">
	<article>{@html html}</article>
</div>

<style>
	.preview {
		box-sizing: border-box;
		height: 100%;
		overflow: auto;
		padding: var(--space-4);
		overflow-wrap: anywhere;
	}

	article {
		max-width: 75ch;
		margin: 0 auto;
		line-height: 1.6;
	}

	article > :global(:first-child) {
		margin-top: 0;
	}

	article > :global(:last-child) {
		margin-bottom: 0;
	}

	article :global(:is(h1, h2, h3, h4, h5, h6)) {
		line-height: 1.25;
	}

	article :global(code) {
		padding: 1px var(--space-1);
		border-radius: var(--radius-md);
		background: var(--surface-2);
		font-family: var(--font-mono);
		font-size: 0.92em;
	}

	article :global(pre) {
		overflow-x: auto;
		padding: var(--space-3);
		border-radius: var(--radius-lg);
		background: var(--surface-1);
	}

	article :global(pre code) {
		padding: 0;
		background: none;
	}

	article :global(blockquote) {
		margin-left: 0;
		padding-left: var(--space-3);
		border-left: var(--border-width) solid var(--border);
		color: var(--muted-fg);
	}

	article :global(table) {
		display: block;
		overflow-x: auto;
		border-collapse: collapse;
	}

	article :global(:is(th, td)) {
		padding: var(--space-1) var(--space-2);
		border: var(--border-width) solid var(--border);
	}

	article :global(a) {
		color: var(--status-working);
	}

	article :global(img) {
		max-width: 100%;
		height: auto;
	}
</style>
