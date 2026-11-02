import { find, length, mark, outputAt, retime, type Event } from '../edit.ts';
import type { Cue } from '../timing.ts';

const focused = (timeline: Event[], name: string) => find(timeline, 'desktop', (event) => event.type === 'focus' && event.name === name).t;

export function canvasKeys(timeline: Event[]) {
	return retime([
		{ t: mark(timeline, 'desktop', 'start'), fixed: false },
		{ t: focused(timeline, 'card'), fixed: true },
		{ t: mark(timeline, 'desktop', 'opened'), fixed: false },
		{ t: focused(timeline, 'versions'), fixed: true },
		{ t: mark(timeline, 'desktop', 'end'), fixed: true }
	], length('canvas'));
}

export function canvasCues(timeline: Event[]): Cue[] {
	const keys = canvasKeys(timeline);
	const at = (foot: number) => outputAt(keys, foot);
	const flip = timeline.find((event) => event.type === 'click' && event.t > focused(timeline, 'versions'))!.t;
	return [
		{ section: 'canvas', t: at(mark(timeline, 'desktop', 'opened')), sound: 'sparkle' },
		{ section: 'canvas', t: at(mark(timeline, 'desktop', 'v2')), sound: 'tick' },
		{ section: 'canvas', t: at(mark(timeline, 'desktop', 'v3')), sound: 'tick' },
		{ section: 'canvas', t: at(flip), sound: 'whoosh' }
	];
}
