<script lang="ts">
	import Command from '#lib/chat/Command.svelte';
	import { commands } from '#lib/chat/shells.ts';
	import type { Item } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';
	import { untrack } from 'svelte';
	import { cubicOut } from 'svelte/easing';
	import { prefersReducedMotion } from 'svelte/motion';
	import { SvelteMap } from 'svelte/reactivity';
	import { slide } from 'svelte/transition';
	import CaretDown from '~icons/ph/caret-down';

	const DELAY = 1500;
	const LINGER = 1000;

	let { items }: { items: Item[] } = $props();

	const all = $derived(commands(items));
	const times = new SvelteMap<string, { since: number; until?: number }>();
	let now = $state(Date.now());

	$effect(() => {
		for (const { id, item } of all) {
			const time = untrack(() => times.get(id));
			if (item.status === 'running' && !time) times.set(id, { since: Date.now() });
			else if (item.status !== 'running' && time && !time.until) times.set(id, { ...time, until: Date.now() });
		}
	});

	const shells = $derived(all.filter(({ id }) => {
		const time = times.get(id);
		return !!time && (time.until ?? now) - time.since >= DELAY && now - (time.until ?? now) < LINGER;
	}));
	const running = $derived(shells.filter(({ item }) => item.status === 'running').length);
	const ticking = $derived([...times.values()].some(({ until }) => until === undefined || now - until < LINGER));

	$effect(() => {
		if (!ticking) return;
		const timer = setInterval(() => (now = Date.now()), 100);
		return () => clearInterval(timer);
	});

	const motion = $derived({ duration: prefersReducedMotion.current ? 0 : 250, easing: cubicOut });

	function grow(node: HTMLElement) {
		const style = getComputedStyle(node);
		const gap = parseFloat(getComputedStyle(node.parentElement!).rowGap) || 0;
		const height = node.getBoundingClientRect().height;
		const [top, bottom, above, below] = [style.paddingTop, style.paddingBottom, style.borderTopWidth, style.borderBottomWidth].map(parseFloat);
		const edge = above + below;
		return {
			...motion,
			css: (t: number) => `overflow: hidden; box-sizing: border-box; opacity: ${t}; height: ${edge + t * (height - edge)}px; padding-top: ${t * top}px; padding-bottom: ${t * bottom}px; margin-bottom: ${(t - 1) * (gap + edge)}px`
		};
	}

	const phone = $derived(form() === 'phone');
	const heading = $derived(running ? `Running shells · ${running}` : 'Shells finished');
</script>

{#snippet list()}
	<div class="shells">
		{#each shells as shell (shell.id)}
			<div class="shell" transition:slide={motion}>
				{#if shell.owner !== 'Main'}<span class="owner">{shell.owner}</span>{/if}
				<Command item={shell.item} />
			</div>
		{/each}
	</div>
{/snippet}

{#if shells.length}
	<section aria-label="Running shells" class:phone transition:grow>
		{#if phone}
			<details>
				<summary><strong>{heading}</strong><span class="caret"><CaretDown /></span></summary>
				{@render list()}
			</details>
		{:else}
			<strong>{heading}</strong>
			{@render list()}
		{/if}
	</section>
{/if}

<style>
	section { min-width: 0; border-radius: var(--radius-lg); padding: var(--space-2) var(--space-3); background: color-mix(in oklab, var(--status-working) 7%, var(--surface-1)); }
	strong { color: var(--status-working); font-size: var(--font-size-sm); font-weight: 600; }
	.shells { max-height: 25vh; overflow: auto; }
	.shell { min-width: 0; }
	.owner { color: var(--muted-fg); font-size: var(--font-size-sm); overflow-wrap: anywhere; }
	.phone { padding: 0; overflow: hidden; }
	.phone summary { display: flex; align-items: center; justify-content: space-between; gap: var(--space-2); min-height: var(--target-min); padding: 0 var(--space-3); list-style: none; cursor: pointer; }
	.phone summary::-webkit-details-marker { display: none; }
	.phone details[open] > summary { border-bottom: var(--border-width) solid var(--border); background: var(--surface-2); }
	.phone .shells { padding: 0 var(--space-3); }
	.caret { display: inline-flex; flex: none; color: var(--muted-fg); transition: transform var(--dur-fast); }
	details[open] > summary > .caret { transform: rotate(180deg); }
</style>
