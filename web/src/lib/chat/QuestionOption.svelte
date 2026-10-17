<script lang="ts">
	import type { Question } from '#lib/device.svelte.ts';

	let { option }: { option: Question['options'][number] } = $props();
</script>

<strong>{option.label}</strong>
<span>{option.description}</span>
{#if option.preview}<span class="preview">{option.preview}</span>{/if}

<style>
	span { color: var(--muted-fg); font-size: var(--font-size-sm); overflow-wrap: anywhere; }
	.preview { max-width: 100%; overflow-x: auto; white-space: pre; font-family: var(--font-mono); color: var(--color-fg); }
</style>
