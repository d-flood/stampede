import { bar, find, mark, outputAt, retime, type Event, type Key } from '../edit.ts';
import type { Cue } from '../timing.ts';

export const beats = [
	{ name: 'clips', bars: 1, label: 'Clips' },
	{ name: 'queue', bars: 1, label: 'Queued messages' },
	{ name: 'rewind', bars: 1, label: 'Rewind' },
	{ name: 'handoff', bars: 2, label: 'Hand off to another Rig' },
	{ name: 'search', bars: 1, label: 'Search' },
	{ name: 'browser', bars: 1, label: 'Browser preview' },
	{ name: 'usage', bars: 1, label: 'Usage windows' }
];

export const offset = (index: number) => beats.slice(0, index).reduce((sum, beat) => sum + beat.bars, 0) * bar;

// Each beat starts at the output time the previous one ends, so the footage cuts over the off-camera setup.
export function extrasKeys(timeline: Event[]): Key[] {
	return beats.flatMap((beat, index) => retime([
		{ t: mark(timeline, 'desktop', beat.name), fixed: false },
		{ t: mark(timeline, 'desktop', `${beat.name}-end`), fixed: false }
	], beat.bars * bar).map((key) => ({ out: key.out + offset(index), foot: key.foot })));
}

export function extrasCues(timeline: Event[]): Cue[] {
	const keys = extrasKeys(timeline);
	const at = (foot: number, sound: Cue['sound']): Cue => ({ section: 'extras', t: outputAt(keys, foot), sound });
	return [
		...beats.slice(1).map((_, index): Cue => ({ section: 'extras', t: offset(index + 1), sound: 'tick' })),
		at(find(timeline, 'desktop', (event) => event.type === 'keys').t, 'tap'),
		at(find(timeline, 'desktop', (event) => event.type === 'focus' && event.name === 'summary').t, 'sparkle')
	];
}
