import { find, length, mark, outputAt, retime, type Event } from '../edit.ts';
import type { Cue } from '../timing.ts';

export function editorKeys(timeline: Event[]) {
	return retime([
		{ t: mark(timeline, 'desktop', 'start'), fixed: true },
		{ t: mark(timeline, 'desktop', 'typed'), fixed: false },
		{ t: mark(timeline, 'desktop', 'diagnosed'), fixed: true },
		{ t: mark(timeline, 'desktop', 'fixed'), fixed: false },
		{ t: mark(timeline, 'desktop', 'end'), fixed: true }
	], length('editor'));
}

export function editorCues(timeline: Event[]): Cue[] {
	const keys = editorKeys(timeline);
	const at = (foot: number) => outputAt(keys, foot);
	const save = find(timeline, 'desktop', (event) => event.type === 'keys' && event.name === 'Ctrl + S').t;
	return [
		{ section: 'editor', t: at(mark(timeline, 'desktop', 'on')), sound: 'sparkle' },
		{ section: 'editor', t: at(mark(timeline, 'desktop', 'fixed')), sound: 'tick' },
		{ section: 'editor', t: at(save), sound: 'tap' }
	];
}
