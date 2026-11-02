import { find, length, mark, outputAt, retime, type Event } from '../edit.ts';
import type { Cue } from '../timing.ts';

const focus = (timeline: Event[], name: string) => find(timeline, 'desktop', (event) => event.type === 'focus' && event.name === name).t;

export function grillKeys(timeline: Event[]) {
	return retime([
		{ t: mark(timeline, 'desktop', 'start'), fixed: true },
		{ t: mark(timeline, 'desktop', 'sent'), fixed: false },
		{ t: focus(timeline, 'question'), fixed: true },
		{ t: mark(timeline, 'desktop', 'answered'), fixed: false },
		{ t: mark(timeline, 'desktop', 'spec'), fixed: true },
		{ t: mark(timeline, 'desktop', 'specced'), fixed: false },
		{ t: mark(timeline, 'desktop', 'specified'), fixed: true },
		{ t: mark(timeline, 'desktop', 'ticketing'), fixed: false },
		{ t: focus(timeline, 'setup'), fixed: true },
		{ t: mark(timeline, 'desktop', 'confirmed'), fixed: false },
		{ t: mark(timeline, 'desktop', 'ready'), fixed: true },
		{ t: mark(timeline, 'desktop', 'end'), fixed: true }
	], length('grill'));
}

export function grillCues(timeline: Event[]): Cue[] {
	const keys = grillKeys(timeline);
	return [
		{ section: 'grill', t: outputAt(keys, focus(timeline, 'question')), sound: 'notify' },
		{ section: 'grill', t: outputAt(keys, focus(timeline, 'card')), sound: 'sparkle' },
		{ section: 'grill', t: outputAt(keys, mark(timeline, 'desktop', 'ready')), sound: 'tick' }
	];
}
