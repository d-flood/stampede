import { openDB, type DBSchema } from 'idb';

export interface Note {
	id: string;
	title: string;
	body: string;
	pinned: boolean;
	updated: number;
}

interface Tidepool extends DBSchema {
	notes: { key: string; value: Note; indexes: { updated: number } };
}

const database = openDB<Tidepool>('tidepool', 1, {
	upgrade(db) {
		db.createObjectStore('notes', { keyPath: 'id' }).createIndex('updated', 'updated');
	}
});

export async function listNotes(): Promise<Note[]> {
	const notes = await (await database).getAllFromIndex('notes', 'updated');
	return notes.reverse().sort((a, b) => Number(b.pinned) - Number(a.pinned));
}

export async function saveNote(note: Note) {
	await (await database).put('notes', { ...note, updated: Date.now() });
}

export async function deleteNote(id: string) {
	await (await database).delete('notes', id);
}

export function createNote(): Note {
	return { id: crypto.randomUUID(), title: 'Untitled', body: '', pinned: false, updated: Date.now() };
}

export function searchNotes(notes: Note[], query: string) {
	const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
	return notes.filter((note) => {
		const text = `${note.title}\n${note.body}`.toLowerCase();
		return terms.every((term) => text.includes(term));
	});
}
