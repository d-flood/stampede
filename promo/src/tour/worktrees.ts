import { find, length, mark, outputAt, retime, type Event } from '../edit.ts';
import type { Cue } from '../timing.ts';

const focus = (timeline: Event[], name: string) => find(timeline, 'desktop', (event) => event.type === 'focus' && event.name === name).t;

export function worktreesKeys(timeline: Event[]) {
	return retime([
		{ t: mark(timeline, 'desktop', 'start'), fixed: true },
		{ t: mark(timeline, 'desktop', 'sent'), fixed: false },
		{ t: focus(timeline, 'chat'), fixed: true },
		{ t: mark(timeline, 'desktop', 'end'), fixed: true }
	], length('worktrees'));
}

export function worktreesCues(timeline: Event[]): Cue[] {
	return [{ section: 'worktrees', t: outputAt(worktreesKeys(timeline), mark(timeline, 'desktop', 'created')), sound: 'sparkle' }];
}
