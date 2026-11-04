<script lang="ts">
	import Text from '#lib/chat/Text.svelte';
	import Media from '#lib/chat/Media.svelte';
	import type { Question } from '#lib/device.svelte.ts';

	let { option }: { option: Question['options'][number] } = $props();
</script>

<strong>{option.label}{#if option.recommended}<span class="recommended">Recommended</span>{/if}</strong>
{#if option.description}<div class="description"><Text item={{ kind: 'text', text: option.description, streaming: false, phase: 'final' }} /></div>{/if}
{#if option.image}<Media item={{ kind: 'media', ...option.image }} />{/if}
{#if option.preview}<span class="preview">{option.preview}</span>{/if}

<style>
	strong { font-weight: 600; overflow-wrap: anywhere; }
	.recommended { margin-left: var(--space-2); color: var(--status-waiting); font-size: var(--font-size-sm); font-weight: 400; white-space: nowrap; }
	.description { --font-size-chat: var(--font-size-sm); color: var(--muted-fg); }
	.description :global(.text > :last-child) { margin-bottom: 0; }
	.preview { display: block; max-width: 100%; overflow-x: auto; white-space: pre; font-family: var(--font-mono); font-size: var(--font-size-sm); color: var(--muted-fg); }
</style>
