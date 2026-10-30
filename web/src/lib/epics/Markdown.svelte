<script lang="ts">
	import { editor as settings } from '#lib/config.svelte.ts';
	import { find } from '#lib/editor/find.ts';
	import { keybindings } from '#lib/editor/keymap.ts';
	import { theme } from '#lib/editor/theme.ts';
	import { form } from '#lib/form.ts';
	import { markdown } from '@codemirror/lang-markdown';
	import { Compartment, EditorState } from '@codemirror/state';
	import { EditorView, keymap } from '@codemirror/view';
	import { basicSetup } from 'codemirror';
	import { onMount } from 'svelte';

	type Props = { text: string; label: string; autofocus?: boolean; onsave(): void };

	let { text = $bindable(), label, autofocus = true, onsave }: Props = $props();

	let container: HTMLDivElement;
	let view = $state.raw<EditorView>();
	const keys = new Compartment();

	onMount(() => {
		view = new EditorView({
			parent: container,
			state: EditorState.create({
				doc: text,
				extensions: [
					keys.of([]),
					keymap.of([{ key: 'Mod-s', run: () => (onsave(), true) }]),
					basicSetup,
					find,
					markdown(),
					EditorView.lineWrapping,
					EditorView.contentAttributes.of({ 'aria-label': label }),
					theme,
					EditorView.updateListener.of((update) => {
						if (update.docChanged) text = update.state.doc.toString();
					})
				]
			})
		});
		if (autofocus && form() === 'desktop') view.focus();
		return () => view?.destroy();
	});

	$effect(() => {
		const current = view;
		if (!current) return;
		let stale = false;
		keybindings(settings.keymap).then((extension) => {
			if (!stale) current.dispatch({ effects: keys.reconfigure(extension) });
		});
		return () => {
			stale = true;
		};
	});
</script>

<div class="markdown" bind:this={container}></div>

<style>
	.markdown {
		flex: 1;
		min-height: 0;
	}
</style>
