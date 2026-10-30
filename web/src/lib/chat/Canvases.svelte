<script lang="ts">
	import { canvasName } from '#lib/canvas.svelte.ts';
	import Menu from '#lib/ui/Menu.svelte';
	import Browsers from '~icons/ph/browsers';

	let { canvases, oncanvas }: { canvases: string[]; oncanvas(canvas: string): void } = $props();
</script>

<section class="canvases" aria-label="Canvases">
	{#if canvases.length === 1}
		<button class="chip" title="Open canvas" onclick={() => oncanvas(canvases[0])}><Browsers /><span class="name">{canvasName(canvases[0])}</span></button>
	{:else}
		<Menu items={[{ heading: 'Canvases', items: canvases.map((canvas) => ({ label: canvasName(canvas), icon: Browsers, onselect: () => oncanvas(canvas) })) }]}>
			{#snippet trigger(props)}
				<button {...props} class="chip"><Browsers /><span>{canvases.length} canvases</span></button>
			{/snippet}
		</Menu>
	{/if}
</section>

<style>
	.canvases { display: flex; flex: 0 1 auto; min-width: 0; font-size: var(--font-size-sm); }
	.chip { display: inline-flex; align-items: center; gap: var(--space-1); min-width: 0; max-width: 16em; min-height: var(--target-min); padding: 0 var(--space-2); border: var(--border-width) solid var(--border); border-radius: calc(var(--target-min) / 2); background: var(--surface-1); color: var(--color-fg); font: inherit; white-space: nowrap; cursor: pointer; }
	.chip:hover { background: var(--hover); }
	.chip :global(svg) { flex: none; color: var(--muted-fg); }
	.name { min-width: 0; overflow: hidden; text-overflow: ellipsis; font-family: var(--font-mono); }
</style>
