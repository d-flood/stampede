import { find, length, mark, outputAt, retime, type Event } from '../edit.ts';
import type { Cue } from '../timing.ts';

const focus = (timeline: Event[], name: string) => find(timeline, 'desktop', (event) => event.type === 'focus' && event.name === name).t;

export function rigsKeys(timeline: Event[]) {
	return retime([
		{ t: mark(timeline, 'desktop', 'start'), fixed: true },
		{ t: focus(timeline, 'editor'), fixed: false },
		{ t: mark(timeline, 'desktop', 'codex'), fixed: false },
		{ t: mark(timeline, 'desktop', 'closed'), fixed: true },
		{ t: focus(timeline, 'menu'), fixed: false },
		{ t: mark(timeline, 'desktop', 'leader'), fixed: true },
		{ t: mark(timeline, 'desktop', 'launched'), fixed: false },
		{ t: mark(timeline, 'desktop', 'end'), fixed: true }
	], length('rigs'));
}

export function rigsCues(timeline: Event[]): Cue[] {
	const keys = rigsKeys(timeline);
	return [
		{ section: 'rigs', t: outputAt(keys, focus(timeline, 'menu')), sound: 'tick' },
		{ section: 'rigs', t: outputAt(keys, mark(timeline, 'desktop', 'launched')), sound: 'tap' }
	];
}
