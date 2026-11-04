<script lang="ts">
	import { editor as settings } from '#lib/config.svelte.ts';
	import { find } from '#lib/editor/find.ts';
	import { keybindings } from '#lib/editor/keymap.ts';
	import { language } from '#lib/editor/languages.ts';
	import { livePreview } from '#lib/editor/live-preview.ts';
	import { theme } from '#lib/editor/theme.ts';
	import { form } from '#lib/form.ts';
	import { Compartment, EditorState } from '@codemirror/state';
	import { EditorView, keymap } from '@codemirror/view';
	import { basicSetup } from 'codemirror';
	import { onMount } from 'svelte';

	type Props = { text: string; label: string; autofocus?: boolean; onsave?(): void; image?: (src: string) => string | null };

	let { text = $bindable(), label, autofocus = true, onsave, image = (src) => (/^https?:/i.test(src) ? src : null) }: Props = $props();

	let container: HTMLDivElement;
	let view = $state.raw<EditorView>();
	const keys = new Compartment();
	const syntax = new Compartment();

	onMount(() => {
		let disposed = false;
		const created = new EditorView({
			parent: container,
			state: EditorState.create({
				doc: text,
				extensions: [
					keys.of([]),
					keymap.of([{ key: 'Mod-s', run: () => (onsave?.(), true) }]),
					basicSetup,
					find,
					syntax.of([]),
					livePreview({ image: (src) => image(src) }),
					EditorView.lineWrapping,
					EditorView.contentAttributes.of({ 'aria-label': label }),
					theme,
					EditorView.updateListener.of((update) => {
						if (update.docChanged) text = update.state.doc.toString();
					})
				]
			})
		});
		view = created;
		language('.md').then((extension) => {
			if (!disposed) created.dispatch({ effects: syntax.reconfigure(extension) });
		});
		if (autofocus && form() === 'desktop') created.focus();
		return () => {
			disposed = true;
			created.destroy();
		};
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
