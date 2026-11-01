import { beforeEach, describe, expect, it } from 'vitest';
import { createNote, deleteNote, listNotes, saveNote, searchNotes } from './notes';

const note = (title: string, body = '', pinned = false) => ({ ...createNote(), title, body, pinned });

describe('notes', () => {
	beforeEach(async () => {
		for (const { id } of await listNotes()) await deleteNote(id);
	});

	it('creates untitled notes', () => {
		expect(createNote()).toMatchObject({ title: 'Untitled', body: '', pinned: false });
	});

	it('lists pinned notes first', async () => {
		await saveNote(note('Groceries'));
		await saveNote(note('Trip plan', '', true));
		expect((await listNotes()).map((n) => n.title)).toEqual(['Trip plan', 'Groceries']);
	});

	it('lists the most recently updated first', async () => {
		await saveNote(note('Older'));
		await saveNote(note('Newer'));
		expect((await listNotes())[0].title).toBe('Newer');
	});

	it('deletes notes', async () => {
		const groceries = note('Groceries');
		await saveNote(groceries);
		await deleteNote(groceries.id);
		expect(await listNotes()).toEqual([]);
	});

	it('searches titles and bodies', () => {
		const notes = [note('Tide tables', 'low tide 6:40'), note('Reading list', 'The Sea Around Us')];
		expect(searchNotes(notes, 'sea').map((n) => n.title)).toEqual(['Reading list']);
	});

	it('matches every search term', () => {
		const notes = [note('Tide tables', 'low tide 6:40'), note('Low battery')];
		expect(searchNotes(notes, 'low tide')).toHaveLength(1);
	});
});
