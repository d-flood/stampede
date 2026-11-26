<script lang="ts">
	import Kbd from '#lib/ui/Kbd.svelte';
	import type { HTMLAttributes } from 'svelte/elements';

	const glyphs: Record<string, string> = { Shift: '⇧' };

	let { keys, ...rest }: HTMLAttributes<HTMLElement> & { keys: string } = $props();
	const shown = $derived(keys.split('+').map((key) => glyphs[key] ?? key));
</script>

<Kbd {...rest}>{#each shown as key, index (index)}{#if index}{' '}{/if}<span>{key}</span>{/each}</Kbd>
