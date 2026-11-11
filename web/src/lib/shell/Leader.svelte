<script lang="ts">
	import { leader, rigs, type Rig } from '#lib/config.svelte.ts';
	import { form } from '#lib/form.ts';
	import FloatingBar from '#lib/ui/FloatingBar.svelte';
	import Kbd from '#lib/ui/Kbd.svelte';

	type Props = { ontab(index: number): void; onnew(): void; onterminal(): void; onfiles(): void; onsearch(): void; onclose(): void; onpalette(): void; onrig(rig: Rig): void; onsplit(): void };

	let { ontab, onnew, onterminal, onfiles, onsearch, onclose, onpalette, onrig, onsplit }: Props = $props();

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
		if (!pending && event.ctrlKey && event.shiftKey && !event.altKey && !event.metaKey && (key === 't' || key === 'o' || key === 'f' || /^Digit[1-9]$/.test(event.code))) {
			event.preventDefault();
			event.stopPropagation();
			if (event.repeat || (event.target instanceof Element && event.target.closest('[role="dialog"], [role="alertdialog"], [role="menu"]'))) return;
			if (key === 't') onterminal();
			else if (key === 'o') onfiles();
			else if (key === 'f') onsearch();
			else {
				const rig = rigs()[Number(event.code.slice(-1)) - 1];
				if (rig) onrig(rig);
			}
			return;
		}
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
	<FloatingBar role="status">
		{#if pending === 'leader'}
			<span><Kbd>1</Kbd>–<Kbd>9</Kbd> Tab</span>
			<span><Kbd>p</Kbd> Palette</span>
			{#if form() === 'desktop'}<span><Kbd>s</Kbd> Split</span>{/if}
			{#if shortcuts.length}<span><Kbd>r</Kbd> Rig</span>{/if}
		{:else}
			{#each shortcuts as rig (rig.name)}
				<span><Kbd>{rig.shortcut}</Kbd> {rig.name}</span>
			{/each}
		{/if}
		<span><Kbd>Esc</Kbd> Cancel</span>
	</FloatingBar>
{/if}

<style>
	span {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		color: var(--muted-fg);
		white-space: nowrap;
	}
</style>
