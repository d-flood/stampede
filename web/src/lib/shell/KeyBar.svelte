<script module lang="ts">
	export function control(data: string): string {
		const code = data.toUpperCase().charCodeAt(0);
		// Flipping bit 6 maps @, A–Z, [, \, ], ^, _ onto 0–31 and ? onto DEL, as a keyboard's Ctrl chord does.
		return code === 63 || (code >= 64 && code <= 95) ? String.fromCharCode(code ^ 64) + data.slice(1) : data;
	}
</script>

<script lang="ts">
	import { keybarExtras } from '#lib/config.svelte.ts';
	import { on } from 'svelte/events';
	import Copy from '~icons/ph/copy';

	type Props = {
		ctrl: boolean;
		application(): boolean;
		onkey(data: string): void;
		oncopy(): void;
	};

	let { ctrl = $bindable(), application, onkey, oncopy }: Props = $props();

	const arrows = [
		['←', 'Left', 'D'],
		['↓', 'Down', 'B'],
		['↑', 'Up', 'A'],
		['→', 'Right', 'C']
	];

	let below = $state(0);

	function dock(bar: HTMLElement) {
		const viewport = window.visualViewport;
		const panel = bar.parentElement;
		if (!viewport || !panel) return;
		const measure = () => {
			below = Math.max(0, panel.getBoundingClientRect().bottom - viewport.offsetTop - viewport.height);
		};
		measure();
		const off = [on(viewport, 'resize', measure), on(viewport, 'scroll', measure)];
		return () => off.forEach((listener) => listener());
	}
</script>

<div
	class="keybar"
	role="toolbar"
	aria-label="Keys"
	tabindex="-1"
	style:margin-bottom="{below}px"
	onmousedown={(event) => event.preventDefault()}
	{@attach dock}
>
	<button onclick={() => onkey('\x1b')}>Esc</button>
	<button aria-pressed={ctrl} onclick={() => (ctrl = !ctrl)}>Ctrl</button>
	<button onclick={() => onkey('\t')}>Tab</button>
	<button aria-label="Shift Tab" onclick={() => onkey('\x1b[Z')}>⇧Tab</button>
	{#each arrows as [label, name, final] (name)}
		<button aria-label={name} onclick={() => onkey(`\x1b${application() ? 'O' : '['}${final}`)}>{label}</button>
	{/each}
	{#each keybarExtras() as extra, index (index)}
		<button onclick={() => onkey(extra)}>{extra}</button>
	{/each}
	<button aria-label="Copy mode" onclick={oncopy}><Copy /></button>
</div>

<style>
	.keybar {
		display: flex;
		flex: none;
		gap: var(--space-1);
		padding: var(--space-1);
		overflow-x: auto;
		border-top: var(--border-width) solid var(--border);
		background: var(--surface-1);
		scrollbar-width: none;
		outline: none;
	}

	button {
		display: inline-grid;
		flex: none;
		place-items: center;
		min-width: var(--target-min);
		min-height: var(--target-min);
		padding: 0 var(--space-3);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-sm);
		background: var(--surface-2);
		color: var(--color-fg);
		font-family: var(--font-mono);
		font-size: var(--font-size-lg);
		white-space: pre;
		cursor: pointer;
		touch-action: manipulation;
	}

	button:active {
		background: var(--hover);
	}

	button[aria-pressed='true'] {
		border-color: var(--status-working);
		background: var(--status-working);
		color: var(--color-bg);
	}
</style>
