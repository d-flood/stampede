<script lang="ts">
	import { fileIcon } from '#lib/editor/languages.ts';

	let { path }: { path: string } = $props();

	const icon = $derived(fileIcon(path));
</script>

<span class="file-icon" style:color={icon.color}>{@html icon.svg}</span>

<style>
	.file-icon {
		display: inline-flex;
		flex: none;
	}

	.file-icon > :global(svg) {
		width: 0.85em;
		height: 0.85em;
	}
</style>
