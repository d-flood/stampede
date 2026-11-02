import { find, length, mark, outputAt, retime, type Event } from '../edit.ts';
import type { Cue } from '../timing.ts';

const focus = (timeline: Event[], name: string) => find(timeline, 'desktop', (event) => event.type === 'focus' && event.name === name).t;

export function delegateKeys(timeline: Event[]) {
	const back = mark(timeline, 'desktop', 'back');
	const leave = timeline.filter((event) => event.type === 'move' && event.box && event.t < back).at(-1)!.t;
	return retime([
		{ t: mark(timeline, 'desktop', 'start'), fixed: true },
		{ t: mark(timeline, 'desktop', 'sent'), fixed: false },
		{ t: focus(timeline, 'dock'), fixed: true },
		{ t: mark(timeline, 'desktop', 'opened'), fixed: false },
		{ t: mark(timeline, 'desktop', 'reported'), fixed: false },
		{ t: leave, fixed: true },
		{ t: back, fixed: false },
		{ t: mark(timeline, 'desktop', 'end'), fixed: true }
	], length('delegate'));
}

export function delegateCues(timeline: Event[]): Cue[] {
	const keys = delegateKeys(timeline);
	return [
		{ section: 'delegate', t: outputAt(keys, focus(timeline, 'dock')), sound: 'sparkle' },
		{ section: 'delegate', t: outputAt(keys, mark(timeline, 'desktop', 'reported')), sound: 'tick' }
	];
}
