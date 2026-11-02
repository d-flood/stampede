import { find, length, mark, outputAt, retime, type Event, type Key, type Section } from './edit.ts';
import { extrasCues } from './tour/extras.ts';
import { grillCues } from './tour/grill.ts';
import { worktreesCues } from './tour/worktrees.ts';
import { delegateCues } from './tour/delegate.ts';
import { terminalsCues } from './tour/terminals.ts';
import { editorCues } from './tour/editor.ts';
import { gitCues } from './tour/git.ts';
import { mediaCues } from './tour/media.ts';
import { canvasCues } from './tour/canvas.ts';
import { rigsCues } from './tour/rigs.ts';
import { lightCues } from './tour/light.ts';

export const scenes = ['chat', 'split', 'epic', 'fleet', 'extras', 'grill', 'worktrees', 'delegate', 'terminals', 'editor', 'git', 'media', 'canvas', 'rigs', 'light'] as const;
export type Timelines = Record<(typeof scenes)[number], Event[]>;

export const lock = { notify: 0.5, open: 2.0, length: 2.5 };

const focus = (timeline: Event[], device: string, name: string) => find(timeline, device, (event) => event.type === 'focus' && event.name === name).t;

export function chatKeys(timeline: Event[]) {
	return retime([
		{ t: mark(timeline, 'desktop', 'start'), fixed: true },
		{ t: mark(timeline, 'desktop', 'sent'), fixed: false },
		{ t: focus(timeline, 'desktop', 'subagent'), fixed: true },
		{ t: mark(timeline, 'desktop', 'scouted'), fixed: false },
		{ t: focus(timeline, 'desktop', 'question'), fixed: true },
		{ t: mark(timeline, 'desktop', 'answered'), fixed: false },
		{ t: focus(timeline, 'desktop', 'approval'), fixed: true },
		{ t: mark(timeline, 'desktop', 'end'), fixed: true }
	], length('chat'));
}

export function phoneKeys(timeline: Event[]) {
	return retime([
		{ t: mark(timeline, 'phone', 'start'), fixed: false },
		{ t: mark(timeline, 'phone', 'end'), fixed: false }
	], length('phone') - lock.length);
}

export function reviewKeys(timeline: Event[]) {
	return retime([
		{ t: mark(timeline, 'desktop', 'review'), fixed: false },
		{ t: mark(timeline, 'desktop', 'reviewed'), fixed: false }
	], length('review'));
}

export function splitKeys(timeline: Event[]) {
	return retime([
		{ t: mark(timeline, 'desktop', 'start'), fixed: false },
		{ t: mark(timeline, 'desktop', 'asked'), fixed: true },
		{ t: mark(timeline, 'desktop', 'end'), fixed: true }
	], length('split'));
}

export function epicKeys(timeline: Event[]) {
	return retime([
		{ t: mark(timeline, 'desktop', 'start'), fixed: true },
		{ t: mark(timeline, 'desktop', 'started'), fixed: false },
		{ t: mark(timeline, 'desktop', 'review'), fixed: true },
		{ t: mark(timeline, 'desktop', 'reviewing'), fixed: false },
		{ t: mark(timeline, 'desktop', 'remediated'), fixed: true },
		{ t: mark(timeline, 'desktop', 'end'), fixed: true }
	], length('epic'));
}

export function fleetKeys(timeline: Event[]) {
	return retime([
		{ t: mark(timeline, 'desktop', 'start'), fixed: false },
		{ t: mark(timeline, 'desktop', 'drawer'), fixed: true },
		{ t: mark(timeline, 'desktop', 'end'), fixed: true }
	], length('fleet'));
}

export type Sound = 'notify' | 'tick' | 'tap' | 'sparkle' | 'whoosh';
export type Cue = { section: Section; t: number; sound: Sound };

export function cues(timelines: Timelines): Cue[] {
	const at = (section: Section, keys: Key[], foot: number, sound: Sound, offset = 0): Cue => ({ section, t: offset + outputAt(keys, foot), sound });
	const { chat, split, epic, fleet, extras, grill, worktrees, delegate, terminals, editor, git, media, canvas, rigs, light } = timelines;
	const clicks = (timeline: Event[], from: number, to: number) => timeline.filter((event) => event.type === 'click' && event.t >= from && event.t <= to).map((event) => event.t);
	const chatting = chatKeys(chat);
	const phone = phoneKeys(chat);
	const review = reviewKeys(chat);
	const epics = epicKeys(epic);
	const fleets = fleetKeys(fleet);
	const commit = clicks(chat, mark(chat, 'desktop', 'review'), mark(chat, 'desktop', 'reviewed')).at(-2)!;
	return [
		at('chat', chatting, focus(chat, 'desktop', 'mockup'), 'sparkle'),
		at('chat', chatting, mark(chat, 'desktop', 'answered'), 'tick'),
		{ section: 'phone', t: lock.notify, sound: 'notify' },
		{ section: 'phone', t: lock.open, sound: 'tick' },
		at('phone', phone, find(chat, 'phone', (event) => event.type === 'tap').t, 'tap', lock.length),
		at('review', review, commit, 'sparkle'),
		at('split', splitKeys(split), mark(split, 'desktop', 'asked'), 'notify'),
		at('epic', epics, mark(epic, 'desktop', 'started'), 'sparkle'),
		at('epic', epics, clicks(epic, mark(epic, 'desktop', 'land'), Infinity).at(-1)!, 'tap'),
		at('fleet', fleets, find(fleet, 'desktop', (event) => event.type === 'keys').t, 'whoosh'),
		{ section: 'themes', t: 0, sound: 'whoosh' },
		...extrasCues(extras),
		...grillCues(grill),
		...worktreesCues(worktrees),
		...delegateCues(delegate),
		...terminalsCues(terminals),
		...editorCues(editor),
		...gitCues(git),
		...mediaCues(media),
		...canvasCues(canvas),
		...rigsCues(rigs),
		...lightCues(light)
	];
}
