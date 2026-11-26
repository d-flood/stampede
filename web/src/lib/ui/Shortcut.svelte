<script lang="ts">
	import Kbd from '#lib/ui/Kbd.svelte';
	import type { HTMLAttributes } from 'svelte/elements';

	const glyphs: Record<string, string> = { Shift: '⇧' };

	let { keys, ...rest }: HTMLAttributes<HTMLElement> & { keys: string } = $props();
	const shown = $derived(keys.split('+').map((key) => glyphs[key] ?? key));
</script>

<kbd {...rest}>{#each shown as key, index (index)}{#if index}{' '}{/if}<Kbd>{key}</Kbd>{/each}</kbd>

<style>
	kbd {
		display: inline-flex;
		flex: none;
		gap: 2px;
		font: inherit;
		white-space: nowrap;
	}
</style>
