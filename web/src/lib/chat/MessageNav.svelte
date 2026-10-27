<script lang="ts">
	import type { Item } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import { on } from 'svelte/events';
	import CaretDown from '~icons/ph/caret-down';
	import CaretUp from '~icons/ph/caret-up';
	import ListNumbers from '~icons/ph/list-numbers';

	let { items, scroller, height = $bindable(0) }: { items: Item[]; scroller?: HTMLElement; height?: number } = $props();
	const messages = $derived(items.flatMap((item, index) => item.kind === 'user' ? [{ index, summary: item.text.trim().split('\n')[0] || `${item.images} attached image${item.images === 1 ? '' : 's'}` }] : []));
	let nav = $state<HTMLElement>();
	let current = $state<number>();
	let above = $state(false);
	let below = $state(false);
	let revealed = $state(false);
	let gutter = $state(0);
	let landing: number | undefined;
	const position = $derived(messages.findIndex(({ index }) => index === current));
	const shown = $derived(revealed && current !== undefined);

	function edge() {
		return scroller!.getBoundingClientRect().top + (nav?.offsetHeight ?? 0);
	}

	function entries() {
		return [...scroller!.querySelectorAll<HTMLElement>('[data-user-message]')].map((node) => ({ node, top: node.getBoundingClientRect().top }));
	}

	function update() {
		const line = edge();
		const list = entries();
		const at = list.findLast(({ top }) => top <= line + 2);
		current = at && Number(at.node.dataset.userMessage);
		above = list.some(({ top }) => top < line - 2);
		below = list.some(({ top }) => top > line + 2) && scroller!.scrollTop < scroller!.scrollHeight - scroller!.clientHeight - 1;
		gutter = scroller!.offsetWidth - scroller!.clientWidth;
	}

	function watch(element: HTMLElement) {
		let last = element.scrollTop;
		const observer = new ResizeObserver(update);
		observer.observe(element);
		if (element.firstElementChild) observer.observe(element.firstElementChild);
		const off = on(element, 'scroll', () => {
			const top = element.scrollTop;
			if (top !== landing && top !== last) revealed = top < last;
			landing = undefined;
			last = top;
			update();
		});
		update();
		return () => { observer.disconnect(); off(); revealed = false; };
	}

	function jump(node?: HTMLElement | null) {
		if (!node || !scroller) return;
		scroller.scrollTop += node.getBoundingClientRect().top - edge();
		landing = scroller.scrollTop;
	}

	function step(direction: -1 | 1) {
		const line = edge();
		const list = entries();
		jump((direction < 0 ? list.findLast(({ top }) => top < line - 2) : list.find(({ top }) => top > line + 2))?.node);
	}
</script>

{#if messages.length && scroller}
	<nav bind:this={nav} bind:offsetHeight={height} aria-label="User messages" class:shown class:phone={form() === 'phone'} style:right="{gutter}px" inert={!shown} {@attach () => watch(scroller!)}>
		<div class="row">
			<Menu items={messages.map((message, number) => ({ label: `${number + 1}. ${message.summary}`, checked: message.index === current, onselect: () => jump(scroller?.querySelector<HTMLElement>(`[data-user-message="${message.index}"]`)) }))}>
				{#snippet trigger(props)}
					<button {...props} class="current" aria-label={position < 0 ? `${messages.length} user messages` : `Message ${position + 1} of ${messages.length}: ${messages[position].summary}`}>
						<ListNumbers aria-hidden="true" />
						{#if position < 0}
							<span class="summary">{messages.length} message{messages.length === 1 ? '' : 's'}</span>
						{:else}
							<span class="count">{position + 1}/{messages.length}</span>
							<span class="summary">{messages[position].summary}</span>
						{/if}
					</button>
				{/snippet}
			</Menu>
			<IconButton label="Previous message" disabled={!above} onclick={() => step(-1)}><CaretUp /></IconButton>
			<IconButton label="Next message" disabled={!below} onclick={() => step(1)}><CaretDown /></IconButton>
		</div>
	</nav>
{/if}

<style>
	nav { position: absolute; z-index: 1; top: 0; left: 0; border-bottom: var(--border-width) solid var(--border); background: var(--surface-1); box-shadow: var(--shadow-float); font-size: var(--font-size-sm); opacity: 0; transform: translateY(-100%); pointer-events: none; transition: transform var(--dur-med) ease, opacity var(--dur-med) ease; }
	nav.shown { opacity: 1; transform: none; pointer-events: auto; }
	.row { display: flex; align-items: center; gap: var(--space-1); max-width: 760px; margin: 0 auto; padding: 0 var(--space-4); }
	.current { display: flex; flex: 1; align-items: center; gap: var(--space-2); min-width: 0; min-height: var(--target-min); padding: 0 var(--space-2); border: none; border-radius: var(--radius-md); background: none; color: var(--color-fg); font: inherit; text-align: left; cursor: pointer; }
	.current:hover, .current[data-state='open'] { background: var(--hover); }
	.current:focus-visible { outline: var(--border-width) solid var(--status-working); }
	.current :global(svg) { flex: none; color: var(--muted-fg); }
	.count { flex: none; color: var(--muted-fg); font-variant-numeric: tabular-nums; }
	.summary { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
	.phone { font-size: var(--font-size-md); }
	.phone .row { padding: 0 var(--space-2); }
</style>
