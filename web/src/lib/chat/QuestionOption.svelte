<script lang="ts">
	import Text from '#lib/chat/Text.svelte';
	import Media from '#lib/chat/Media.svelte';
	import type { Question } from '#lib/device.svelte.ts';

	let { option, rich = false, showLabel = true }: { option: Question['options'][number]; rich?: boolean; showLabel?: boolean } = $props();
</script>

{#if showLabel}<strong>{option.label}{#if option.recommended}<span class="recommended">Recommended</span>{/if}</strong>{/if}
{#if rich}<Text item={{ kind: 'text', text: option.description, streaming: false, phase: 'final' }} />{:else}<span>{option.description}</span>{/if}
{#if option.image}<Media item={{ kind: 'media', ...option.image }} />{/if}
{#if option.preview}<span class="preview">{option.preview}</span>{/if}

<style>
	.recommended { margin-left: var(--space-2); color: var(--status-waiting); }
	span { color: var(--muted-fg); font-size: var(--font-size-sm); overflow-wrap: anywhere; }
	.preview { max-width: 100%; overflow-x: auto; white-space: pre; font-family: var(--font-mono); color: var(--color-fg); }
</style>
