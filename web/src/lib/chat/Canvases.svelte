<script lang="ts">
	import { canvasName } from '#lib/canvas.svelte.ts';
	import Chip from '#lib/ui/Chip.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import Browsers from '~icons/ph/browsers';

	let { canvases, oncanvas }: { canvases: string[]; oncanvas(canvas: string): void } = $props();
</script>

<section class="canvases" aria-label="Canvases">
	{#if canvases.length === 1}
		<Chip title="Open canvas" onclick={() => oncanvas(canvases[0])}><Browsers /><span class="name">{canvasName(canvases[0])}</span></Chip>
	{:else}
		<Menu items={[{ heading: 'Canvases', items: canvases.map((canvas) => ({ label: canvasName(canvas), icon: Browsers, onselect: () => oncanvas(canvas) })) }]}>
			{#snippet trigger(props)}
				<Chip {...props}><Browsers /><span>{canvases.length} canvases</span></Chip>
			{/snippet}
		</Menu>
	{/if}
</section>

<style>
	.canvases { display: flex; flex: 0 1 auto; min-width: 0; max-width: 16em; }
	.name { min-width: 0; overflow: hidden; text-overflow: ellipsis; font-family: var(--font-mono); }
</style>
