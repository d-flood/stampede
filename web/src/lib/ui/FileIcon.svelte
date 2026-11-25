<script lang="ts">
	import { fileIcon } from '#lib/editor/languages.ts';

	let { path }: { path: string } = $props();

	const svg = $derived(fileIcon(path));
</script>

<span class="file-icon">{@html svg}</span>

<style>
	.file-icon {
		display: inline-flex;
		flex: none;
		color: var(--muted-fg);
	}

	.file-icon > :global(svg) {
		width: 1em;
		height: 1em;
	}
</style>
