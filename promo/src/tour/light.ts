import { find, length, mark, outputAt, retime, type Event } from '../edit.ts';
import type { Cue } from '../timing.ts';

export function lightKeys(timeline: Event[]) {
	return retime([
		{ t: mark(timeline, 'desktop', 'start'), fixed: false },
		{ t: mark(timeline, 'desktop', 'drawer'), fixed: true },
		{ t: mark(timeline, 'desktop', 'lsp'), fixed: false },
		{ t: mark(timeline, 'desktop', 'end'), fixed: false }
	], length('light'));
}

export function lightCues(timeline: Event[]): Cue[] {
	const keys = lightKeys(timeline);
	return [
		{ section: 'light', t: outputAt(keys, find(timeline, 'desktop', (event) => event.type === 'keys').t), sound: 'whoosh' }
	];
}
