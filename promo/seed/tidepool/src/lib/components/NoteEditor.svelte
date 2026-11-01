<script lang="ts">
	import { saveNote, type Note } from '$lib/notes';

	let { note }: { note: Note } = $props();
	let timer: ReturnType<typeof setTimeout>;

	function changed() {
		clearTimeout(timer);
		timer = setTimeout(() => saveNote(note), 400);
	}
</script>

<article class="editor">
	<header>
		<input class="title" bind:value={note.title} oninput={changed} aria-label="Title" />
		<button class:pinned={note.pinned} onclick={() => { note.pinned = !note.pinned; changed(); }}>
			{note.pinned ? 'Pinned' : 'Pin'}
		</button>
	</header>
	<textarea bind:value={note.body} oninput={changed} placeholder="Start writing…"></textarea>
</article>

<style>
	.editor {
		display: grid;
		grid-template-rows: auto 1fr;
		gap: 1rem;
		padding: 2rem;
	}
	header {
		display: flex;
		align-items: center;
		gap: 1rem;
	}
	.title {
		flex: 1;
		font: 600 1.75rem/1.2 var(--font-display);
		border: none;
		background: none;
	}
	.pinned {
		color: var(--tide);
	}
	textarea {
		resize: none;
		border: none;
		font: 1.05rem/1.6 var(--font-body);
	}
</style>
