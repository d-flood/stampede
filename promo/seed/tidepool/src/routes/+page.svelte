<script lang="ts">
	import { onMount } from 'svelte';
	import NoteEditor from '$lib/components/NoteEditor.svelte';
	import NoteList from '$lib/components/NoteList.svelte';
	import { createNote, listNotes, saveNote, searchNotes, type Note } from '$lib/notes';

	let notes = $state<Note[]>([]);
	let query = $state('');
	let selected = $state<string>();

	const visible = $derived(searchNotes(notes, query));
	const current = $derived(notes.find((note) => note.id === selected));

	onMount(async () => {
		notes = await listNotes();
		selected = notes[0]?.id;
	});

	async function add() {
		const note = createNote();
		await saveNote(note);
		notes = [note, ...notes];
		selected = note.id;
	}
</script>

<div class="app">
	<aside>
		<header>
			<h1>Tidepool</h1>
			<button onclick={add} aria-label="New note">+</button>
		</header>
		<input type="search" bind:value={query} placeholder="Search notes" />
		<NoteList notes={visible} bind:selected />
	</aside>
	{#if current}
		<NoteEditor note={current} />
	{/if}
</div>

<style>
	.app {
		display: grid;
		grid-template-columns: 20rem 1fr;
		height: 100dvh;
	}
	aside {
		border-inline-end: 1px solid var(--border);
		overflow-y: auto;
	}
	header {
		display: flex;
		justify-content: space-between;
		padding: 1rem;
	}
	input[type='search'] {
		margin: 0 1rem 0.5rem;
		width: calc(100% - 2rem);
	}
</style>
