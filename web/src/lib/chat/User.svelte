<script lang="ts">
	import Text from '#lib/chat/Text.svelte';
	import type { Item } from '#lib/device.svelte.ts';
	import CaretRight from '~icons/ph/caret-right';

	let { item, first }: { item: Extract<Item, { kind: 'user' }>; first: boolean } = $props();

	const lines = $derived(item.text.split('\n'));

	const markdown = (text: string) => ({ kind: 'text', text, streaming: false, phase: 'final' }) as const;
</script>

{#if lines.length > 12}
	<details class="user">
		<summary>
			<span class="first">
				{#if first}<span class="label">First prompt</span>{' · '}{/if}{lines[0]}
			</span>
			<span class="label">{lines.length} lines</span>
			<span class="caret"><CaretRight /></span>
		</summary>
		<div class="body"><Text item={markdown(item.text)} /></div>
	</details>
{:else if item.template}
	<div class="user"><span class="template">/{item.template}</span>{' '}<Text item={markdown(item.text.slice(item.template.length + 1))} /></div>
{:else}
	<div class="user"><Text item={markdown(item.text)} /></div>
{/if}

{#if item.images}<div class="label images">{item.images} image{item.images === 1 ? '' : 's'}</div>{/if}

<style>
	.user {
		align-self: flex-end;
		max-width: 85%;
		padding: var(--space-3) var(--space-4);
		border: var(--border-width) solid color-mix(in oklab, var(--ansi-4) 38%, var(--color-bg));
		border-radius: var(--radius-lg) var(--radius-lg) var(--radius-sm) var(--radius-lg);
		background: color-mix(in oklab, var(--ansi-4) 16%, var(--color-bg));
	}

	details.user {
		padding-block: 0;
	}

	summary {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-4);
		min-height: var(--target-min);
		list-style: none;
		cursor: pointer;
		font-size: 0.92em;
	}

	summary::-webkit-details-marker {
		display: none;
	}

	.caret {
		display: inline-flex;
		flex: none;
		width: 1em;
		color: var(--muted-fg);
		transition: transform var(--dur-fast);
	}

	details[open] > summary > .caret {
		transform: rotate(90deg);
	}

	.first {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.label {
		color: var(--muted-fg);
	}

	.template {
		float: left;
		margin-right: 1ch;
		font-family: var(--font-mono);
		font-weight: 600;
	}

	.images {
		align-self: flex-end;
	}

	.body {
		padding-bottom: var(--space-3);
	}
</style>
