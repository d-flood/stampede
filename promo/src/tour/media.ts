import { find, length, mark, outputAt, retime, type Event } from '../edit.ts';
import type { Cue } from '../timing.ts';

const focused = (timeline: Event[], name: string) => find(timeline, 'desktop', (event) => event.type === 'focus' && event.name === name).t;

export function mediaKeys(timeline: Event[]) {
	return retime([
		{ t: mark(timeline, 'desktop', 'start'), fixed: false },
		{ t: focused(timeline, 'image'), fixed: false },
		{ t: focused(timeline, 'video'), fixed: true },
		{ t: mark(timeline, 'desktop', 'end'), fixed: true }
	], length('media'));
}

export function mediaCues(timeline: Event[]): Cue[] {
	const keys = mediaKeys(timeline);
	return [
		{ section: 'media', t: outputAt(keys, focused(timeline, 'image')), sound: 'sparkle' },
		{ section: 'media', t: outputAt(keys, mark(timeline, 'desktop', 'play')), sound: 'tap' }
	];
}
