import { length, mark, outputAt, retime, type Event } from '../edit.ts';
import type { Cue } from '../timing.ts';

export function gitKeys(timeline: Event[]) {
	return retime([
		{ t: mark(timeline, 'desktop', 'start'), fixed: true },
		{ t: mark(timeline, 'desktop', 'published'), fixed: false },
		{ t: mark(timeline, 'desktop', 'end'), fixed: true }
	], length('git'));
}

export function gitCues(timeline: Event[]): Cue[] {
	const keys = gitKeys(timeline);
	const at = (name: string) => outputAt(keys, mark(timeline, 'desktop', name));
	return [
		{ section: 'git', t: at('branched'), sound: 'tick' },
		{ section: 'git', t: at('committed'), sound: 'sparkle' },
		{ section: 'git', t: at('published'), sound: 'whoosh' }
	];
}
