<script lang="ts">
	import type { Device } from '#lib/device.svelte.ts';
	import type { OpenFile } from '#lib/files.svelte.ts';
	import ImagePreview from '#lib/shell/ImagePreview.svelte';

	let { file, device, active }: { file: OpenFile; device: Device; active: boolean } = $props();
</script>

<div class="image" role="tabpanel" hidden={!active}>
	<div class="toolbar">
		<span class="path" title={file.path}>{file.path}</span>
		{#if file.deleted}<span class="deleted">deleted on disk</span>{/if}
	</div>
	<div class="content"><ImagePreview {file} {device} {active} /></div>
</div>

<style>
	.image {
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.image[hidden] {
		display: none;
	}

	.toolbar {
		display: flex;
		flex: none;
		align-items: center;
		gap: var(--space-2);
		min-height: var(--target-min);
		padding: 0 var(--space-3);
		border-bottom: var(--border-width) solid var(--border);
	}

	.path {
		flex: 1;
		overflow: hidden;
		color: var(--muted-fg);
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.deleted {
		flex: none;
		color: var(--status-waiting);
		font-size: var(--font-size-sm);
	}

	.content {
		flex: 1;
		min-height: 0;
	}
</style>
