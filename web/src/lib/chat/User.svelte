<script lang="ts">
	import Text from '#lib/chat/Text.svelte';
	import type { Item } from '#lib/device.svelte.ts';
	import CaretRight from '~icons/ph/caret-right';
	import ArrowCounterClockwise from '~icons/ph/arrow-counter-clockwise';
	import DotsThree from '~icons/ph/dots-three-bold';
	import Menu from '#lib/ui/Menu.svelte';

	let { item, first, index, number, onrewind }: { item: Extract<Item, { kind: 'user' }>; first: boolean; index: number; number: number; onrewind?(index: number): void } = $props();

	const lines = $derived(item.text.split('\n'));

	const markdown = (text: string) => ({ kind: 'text', text, streaming: false, phase: 'final' }) as const;
</script>

{#snippet more()}
	{#if onrewind}
		<Menu items={[{ label: 'Rewind…', icon: ArrowCounterClockwise, onselect: () => onrewind(index) }]}>
			{#snippet trigger(props)}
				<button {...props} class="more" aria-label="Actions for message {number}"><DotsThree aria-hidden="true" /></button>
			{/snippet}
		</Menu>
	{/if}
{/snippet}

<section class="message" data-user-message={index} aria-label="User message {number}">
	{#if item.text}
		<div class="bubble">
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
			{@render more()}
		</div>
	{/if}

	{#if item.images}
		<div class="images" aria-label="Attached images">
			{#if !item.text}{@render more()}{/if}
			<span class="label">{item.images} image{item.images === 1 ? '' : 's'}</span>
			{#each item.imageUrls ?? [] as src, index}
				<a href={src} target="_blank" rel="noreferrer" aria-label="Open attached image {index + 1}">
					<img {src} alt="Attached image {index + 1}" loading="lazy" />
				</a>
			{/each}
		</div>
	{/if}

</section>

<style>
	.message { display: flex; flex-direction: column; gap: var(--space-2); min-width: 0; }
	.bubble { position: relative; align-self: flex-end; max-width: 85%; }
	.more { position: absolute; bottom: 0; left: 0; display: grid; place-items: center; width: 24px; height: 24px; padding: 0; border: var(--border-width) solid var(--border); border-radius: 50%; background: var(--surface-1); color: var(--muted-fg); font-size: var(--font-size-sm); cursor: pointer; transform: translate(-50%, 50%); }
	.more::before { content: ''; position: absolute; inset: calc((24px - var(--target-min)) / 2); border-radius: 50%; }
	.more:hover, .more[aria-expanded='true'] { background: var(--hover); color: var(--color-fg); }
	.more:focus-visible { outline: var(--border-width) solid var(--accent); }
	@media (hover: hover) {
		.more { opacity: 0; transition: opacity var(--dur-med) var(--ease-out); }
		.message:hover .more, .message:focus-within .more, .more[aria-expanded='true'] { opacity: 1; }
	}
	@media (hover: none) {
		.more { width: 20px; height: 20px; font-size: 12px; }
		.more::before { inset: calc((20px - var(--target-min)) / 2); }
	}
	.user {
		padding: var(--space-3) var(--space-4);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg) var(--radius-lg) var(--radius-sm) var(--radius-lg);
		background: var(--surface-2);
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
		position: relative;
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		align-self: flex-end;
		gap: var(--space-2);
		max-width: 85%;
	}

	.images a {
		display: block;
		max-width: 100%;
	}

	.images img {
		display: block;
		max-width: 100%;
		max-height: min(24rem, 50dvh);
		border-radius: var(--radius-md);
		object-fit: contain;
	}

	.body {
		padding-bottom: var(--space-3);
	}
</style>
