<script lang="ts">
	import type { HTMLAttributes } from 'svelte/elements';

	const glyphs: Record<string, string> = { Shift: '⇧' };

	let { keys, ...rest }: HTMLAttributes<HTMLElement> & { keys: string } = $props();
	const shown = $derived(keys.split('+').map((key) => glyphs[key] ?? key).join(' '));
</script>

<kbd {...rest}>{shown}</kbd>

<style>
	kbd {
		flex: none;
		color: var(--muted-fg);
		font: inherit;
		font-size: var(--font-size-sm);
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}
</style>
