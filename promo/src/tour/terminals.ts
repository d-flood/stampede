import { find, length, mark, outputAt, retime, type Event } from '../edit.ts';
import type { Cue } from '../timing.ts';

const focus = (timeline: Event[], name: string) => find(timeline, 'desktop', (event) => event.type === 'focus' && event.name === name).t;

export function terminalsKeys(timeline: Event[]) {
	return retime([
		{ t: mark(timeline, 'desktop', 'start'), fixed: true },
		{ t: mark(timeline, 'desktop', 'sent'), fixed: false },
		{ t: mark(timeline, 'desktop', 'staged'), fixed: true },
		{ t: mark(timeline, 'desktop', 'ran'), fixed: false },
		{ t: focus(timeline, 'output'), fixed: false },
		{ t: mark(timeline, 'desktop', 'end'), fixed: true }
	], length('terminals'));
}

export function terminalsCues(timeline: Event[]): Cue[] {
	const keys = terminalsKeys(timeline);
	return [
		{ section: 'terminals', t: outputAt(keys, mark(timeline, 'desktop', 'staged')), sound: 'notify' },
		{ section: 'terminals', t: outputAt(keys, mark(timeline, 'desktop', 'ran')), sound: 'tap' }
	];
}
