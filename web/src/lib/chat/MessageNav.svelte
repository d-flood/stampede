<script lang="ts">
	import type { Item } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';
	import Button from '#lib/ui/Button.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import { on } from 'svelte/events';
	import CaretDown from '~icons/ph/caret-down';
	import CaretUp from '~icons/ph/caret-up';

	let { items, scroller }: { items: Item[]; scroller?: HTMLElement | null } = $props();
	const messages = $derived(items.flatMap((item, index) => item.kind === 'user' ? [{ index, summary: item.text.trim().split('\n')[0] || `${item.images} attached image${item.images === 1 ? '' : 's'}` }] : []));
	let nav = $state<HTMLElement>();
	let current = $state<number>();
	let above = $state(false);
	let below = $state(false);
	let away = $state(false);
	let atTop = $state(false);
	let gutter = $state(0);
	let inset = 0;
	const phone = $derived(form() === 'phone');
	const position = $derived(messages.findIndex(({ index }) => index === current));
	const shown = $derived(!atTop && away);
	const label = $derived(position < 0 ? `${messages.length} user messages` : `Message ${position + 1} of ${messages.length}: ${messages[position].summary}`);
	const menu = $derived(messages.map((message, number) => ({ label: `${number + 1}. ${message.summary}`, checked: message.index === current, onselect: () => jump(scroller?.querySelector<HTMLElement>(`[data-user-message="${message.index}"]`)) })));

	function bottom() {
		return nav ? nav.offsetTop + nav.offsetHeight : 0;
	}

	/** Where a message rests once reached: under the phone's floating pill, or at the transcript's own top padding. */
	function edge() {
		return phone ? (nav?.offsetParent ?? scroller!).getBoundingClientRect().top + bottom() : scroller!.getBoundingClientRect().top + inset;
	}

	function entries() {
		return [...scroller!.querySelectorAll<HTMLElement>('[data-user-message]')].map((node) => ({ node, top: node.getBoundingClientRect().top }));
	}

	function update() {
		const line = edge();
		const list = entries();
		const at = list.findLast(({ top }) => top <= line + 2);
		current = at && Number(at.node.dataset.userMessage);
		above = list.some(({ top }) => top < line - 2) && scroller!.scrollTop > 0;
		below = list.some(({ top }) => top > line + 2) && scroller!.scrollTop < scroller!.scrollHeight - scroller!.clientHeight - 1;
		gutter = scroller!.offsetWidth - scroller!.clientWidth;
	}

	function watch(element: HTMLElement) {
		inset = parseFloat(getComputedStyle(element).paddingTop);
		const observer = new ResizeObserver(update);
		observer.observe(element);
		if (element.firstElementChild) observer.observe(element.firstElementChild);
		// Only scrolling moves `away`, so growth that the transcript re-pins to the bottom never flashes the pill.
		const measure = () => {
			away = element.scrollTop < element.scrollHeight - element.clientHeight - 2;
			atTop = element.scrollTop < bottom();
		};
		const off = on(element, 'scroll', () => {
			measure();
			update();
		});
		measure();
		update();
		return () => { observer.disconnect(); off(); away = false; };
	}

	function jump(node?: HTMLElement | null) {
		if (!node || !scroller) return;
		scroller.scrollTop += node.getBoundingClientRect().top - edge();
	}

	function step(direction: -1 | 1) {
		const line = edge();
		const list = entries();
		jump((direction < 0 ? list.findLast(({ top }) => top < line - 2) : list.find(({ top }) => top > line + 2))?.node);
	}
</script>

{#if phone && messages.length && scroller}
	<nav bind:this={nav} aria-label="User messages" class="pill" class:shown style:--gutter="{gutter}px" inert={!shown} {@attach () => watch(scroller!)}>
		<IconButton label="Previous message" disabled={!above} onclick={() => step(-1)}><CaretUp /></IconButton>
		<Menu items={menu}>
			{#snippet trigger(props)}
				<button {...props} class="current" aria-label={label}>
					{#if position < 0}{messages.length} message{messages.length === 1 ? '' : 's'}{:else}<span class="count">{position + 1}/{messages.length}</span>{/if}
				</button>
			{/snippet}
		</Menu>
		<IconButton label="Next message" disabled={!below} onclick={() => step(1)}><CaretDown /></IconButton>
	</nav>
{:else if !phone && messages.length > 1 && scroller}
	<nav aria-label="User messages" class="controls" {@attach () => watch(scroller!)}>
		<Menu items={menu} align="end">
			{#snippet trigger(props)}
				<Button {...props} variant="ghost" size="sm" aria-label={label} title={label}>
					<span class="count">{position < 0 ? '–' : position + 1}/{messages.length}</span>
				</Button>
			{/snippet}
		</Menu>
		<span class="steps">
			<Button variant="ghost" size="sm" aria-label="Previous message" title="Previous message" disabled={!above} onclick={() => step(-1)}><CaretUp /></Button>
			<Button variant="ghost" size="sm" aria-label="Next message" title="Next message" disabled={!below} onclick={() => step(1)}><CaretDown /></Button>
		</span>
	</nav>
{/if}

<style>
	.controls { display: flex; flex: none; align-items: center; }
	.steps { display: contents; }
	/* A narrow pane keeps its Tabs readable; the count's menu still reaches every message. */
	@container (width < 26rem) { .steps { display: none; } }
	.count { font-variant-numeric: tabular-nums; }
	.pill { position: absolute; z-index: 2; top: var(--space-2); right: calc(var(--gutter) + var(--space-2)); display: flex; align-items: center; border: var(--border-width) solid var(--border); border-radius: var(--radius-pill); background: var(--color-bg); box-shadow: var(--shadow-float); opacity: 0; transform: translateY(calc(-1 * var(--space-2))); pointer-events: none; transition: transform var(--dur-med) ease, opacity var(--dur-med) ease; }
	.pill.shown { opacity: 1; transform: none; pointer-events: auto; }
	.pill :global(button) { border-radius: var(--radius-pill); }
	.current { display: flex; flex: none; align-items: center; justify-content: center; min-width: var(--target-min); min-height: var(--target-min); padding: 0 var(--space-1); border: none; background: none; color: var(--color-fg); font: inherit; cursor: pointer; }
	.current:hover, .current[data-state='open'] { background: var(--hover); }
	.current:focus-visible { outline: var(--focus-ring); outline-offset: var(--focus-inset); }
	.pill .count { color: var(--muted-fg); }
</style>
