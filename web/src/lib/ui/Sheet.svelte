<script lang="ts">
	import { scrolls, swipe } from '#lib/swipe.ts';
	import type { Snippet } from 'svelte';

	type Props = {
		label: string;
		side: 'left' | 'right';
		open: boolean;
		/** How far a swipe on the page beneath has drawn the closed Sheet in. */
		pull?: number;
		onclose(): void;
		children: Snippet;
	};

	let { label, side, open, pull = 0, onclose, children }: Props = $props();
	let push = $state(0);
	let width = $state(0);
	const away = $derived(side === 'left' ? -1 : 1);
	const moving = $derived(!!pull || !!push);
	const hidden = $derived(open ? Math.min(push, width) : Math.max(width - pull, 0));
</script>

<div class="scrim" class:shown={open || moving} class:moving style:opacity={moving && width ? 1 - hidden / width : undefined} aria-hidden="true" onclick={onclose}></div>
<section
	aria-label={label}
	class="sheet {side}"
	class:open
	class:moving
	tabindex="-1"
	inert={!open}
	style:translate={moving ? `${away * hidden}px` : undefined}
	bind:clientWidth={width}
	{@attach (node) => {
		if (open) node.focus({ preventScroll: true });
	}}
	{@attach swipe({
		share: 0.25,
		direction: away,
		allow: (event) => !scrolls(event),
		drag: (distance) => (push = Math.abs(distance)),
		release: (_, committed) => {
			push = 0;
			if (committed) onclose();
		}
	})}
>
	{@render children()}
</section>

<style>
	.scrim {
		position: fixed;
		inset: 0;
		z-index: 5;
		background: color-mix(in oklab, var(--ansi-0) 45%, transparent);
		opacity: 0;
		pointer-events: none;
		transition: opacity var(--dur-med) var(--ease-out);
	}

	.scrim.shown {
		opacity: 1;
		pointer-events: auto;
	}

	.sheet {
		position: fixed;
		top: 0;
		bottom: 0;
		z-index: 5;
		display: flex;
		flex-direction: column;
		width: min(85vw, 24rem);
		background: var(--color-bg);
		outline: none;
		visibility: hidden;
		transition:
			translate var(--dur-med) var(--ease-out),
			box-shadow var(--dur-med),
			visibility 0s var(--dur-med);
	}

	.left {
		left: 0;
		border: var(--border-width) solid var(--border);
		border-left: none;
		border-radius: 0 var(--radius-xl) var(--radius-xl) 0;
		translate: -100%;
	}

	.right {
		right: 0;
		border: var(--border-width) solid var(--border);
		border-right: none;
		border-radius: var(--radius-xl) 0 0 var(--radius-xl);
		translate: 100%;
	}

	.open,
	.moving {
		box-shadow: var(--shadow-float);
		visibility: visible;
	}

	.open {
		translate: none;
		transition:
			translate var(--dur-med) var(--ease-out),
			box-shadow var(--dur-med);
	}

	.moving {
		transition: none;
	}
</style>
