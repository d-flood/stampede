<script lang="ts">
	import { fileIcon } from '#lib/editor/languages.ts';

	let { path }: { path: string } = $props();
</script>

{@html fileIcon(path)}
