<script lang="ts">
	import { leader, rigs, type Rig } from '#lib/config.svelte.ts';
	import { form } from '#lib/form.ts';

	type Props = { ontab(index: number): void; onnew(): void; onclose(): void; onpalette(): void; onrig(rig: Rig): void; onsplit(): void };

	let { ontab, onnew, onclose, onpalette, onrig, onsplit }: Props = $props();

	const modifiers = ['Control', 'Shift', 'Alt', 'AltGraph', 'Meta'];
	let pending = $state<'leader' | 'rig'>();

	const shortcuts = $derived(rigs().filter((rig) => rig.shortcut));
	const chord = $derived.by(() => {
		const parts = leader().toLowerCase().split('+');
		const key = parts.pop();
		return { key: key === 'space' ? ' ' : key, mods: ['ctrl', 'alt', 'shift', 'meta'].map((mod) => parts.includes(mod)) };
	});

	function pressed(event: KeyboardEvent) {
		const mods = [event.ctrlKey, event.altKey, event.shiftKey, event.metaKey];
		return event.key.toLowerCase() === chord.key && mods.every((held, n) => held === chord.mods[n]);
	}

	// Capture on window runs before ghostty-web's and CodeMirror's own keydown
	// handlers, so neither the PTY nor the editor sees the leader or its next key.
	function keydown(event: KeyboardEvent) {
		if (event.isComposing || event.defaultPrevented) return;
		const key = event.key.toLowerCase();
		if (!pending && (event.ctrlKey !== event.metaKey) && !event.altKey && !event.shiftKey && (key === 't' || key === 'w')) {
			event.preventDefault();
			event.stopPropagation();
			if (event.repeat || (event.target instanceof Element && event.target.closest('[role="dialog"], [role="alertdialog"], [role="menu"]'))) return;
			if (key === 't') onnew();
			else onclose();
			return;
		}
		if (!pending && !pressed(event)) return;
		event.preventDefault();
		event.stopPropagation();
		if (!pending) pending = 'leader';
		else if (!event.repeat && !modifiers.includes(event.key)) act(event.key);
	}

	function act(key: string) {
		const step = pending;
		pending = undefined;
		if (step === 'rig') {
			const rig = shortcuts.find((rig) => rig.shortcut === key);
			if (rig) onrig(rig);
		} else if (/^[1-9]$/.test(key)) ontab(Number(key) - 1);
		else if (key === 'p') onpalette();
		else if (key === 's' && form() === 'desktop') onsplit();
		else if (key === 'r' && shortcuts.length) pending = 'rig';
	}
</script>

<svelte:window onkeydowncapture={keydown} onblur={() => (pending = undefined)} />

{#if pending}
	<div class="hint" role="status">
		{#if pending === 'leader'}
			<span><kbd>1</kbd>–<kbd>9</kbd> Tab</span>
			<span><kbd>p</kbd> Palette</span>
			{#if form() === 'desktop'}<span><kbd>s</kbd> Split</span>{/if}
			{#if shortcuts.length}<span><kbd>r</kbd> Rig</span>{/if}
		{:else}
			{#each shortcuts as rig (rig.name)}
				<span><kbd>{rig.shortcut}</kbd> {rig.name}</span>
			{/each}
		{/if}
		<span><kbd>Esc</kbd> Cancel</span>
	</div>
{/if}

<style>
	.hint {
		position: fixed;
		bottom: var(--space-6);
		left: 50%;
		z-index: 60;
		display: flex;
		flex-wrap: wrap;
		justify-content: center;
		gap: var(--space-2) var(--space-4);
		box-sizing: border-box;
		max-width: calc(100vw - 2 * var(--space-4));
		padding: var(--space-2) var(--space-3);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-2);
		box-shadow: var(--shadow-float);
		color: var(--muted-fg);
		font-family: var(--font-ui);
		font-size: var(--font-size-md);
		transform: translateX(-50%);
	}

	span {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		white-space: nowrap;
	}

	kbd {
		min-width: 1.4em;
		padding: 0 var(--space-1);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-sm);
		background: var(--color-bg);
		color: var(--color-fg);
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		line-height: 1.6;
		text-align: center;
	}
</style>
