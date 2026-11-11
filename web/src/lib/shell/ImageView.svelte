<script lang="ts">
	import type { Device } from '#lib/device.svelte.ts';
	import type { OpenFile } from '#lib/files.svelte.ts';
	import ImagePreview from '#lib/shell/ImagePreview.svelte';
	import Toolbar from '#lib/ui/Toolbar.svelte';

	let { file, device, active }: { file: OpenFile; device: Device; active: boolean } = $props();
</script>

<div class="image" role="tabpanel" hidden={!active}>
	<Toolbar path={file.path} deleted={file.deleted} />
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




	.content {
		flex: 1;
		min-height: 0;
	}
</style>
