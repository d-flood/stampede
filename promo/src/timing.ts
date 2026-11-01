import { find, length, mark, outputAt, retime, start, type Event, type Key, type Section } from './edit.ts';

export const scenes = ['chat', 'split', 'epic', 'fleet'] as const;
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
		{ t: mark(timeline, 'desktop', 'review'), fixed: false },
		{ t: mark(timeline, 'desktop', 'land'), fixed: true },
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

export function cues(timelines: Timelines): { t: number; sound: Sound }[] {
	const at = (section: Section, keys: Key[], foot: number, offset = 0) => start(section) + offset + outputAt(keys, foot);
	const { chat, split, epic, fleet } = timelines;
	const clicks = (timeline: Event[], from: number, to: number) => timeline.filter((event) => event.type === 'click' && event.t >= from && event.t <= to).map((event) => event.t);
	const chatting = chatKeys(chat);
	const phone = phoneKeys(chat);
	const review = reviewKeys(chat);
	const epics = epicKeys(epic);
	const fleets = fleetKeys(fleet);
	const commit = clicks(chat, mark(chat, 'desktop', 'review'), mark(chat, 'desktop', 'reviewed')).at(-2)!;
	return [
		{ t: at('chat', chatting, focus(chat, 'desktop', 'mockup')), sound: 'sparkle' },
		{ t: at('chat', chatting, mark(chat, 'desktop', 'answered')), sound: 'tick' },
		{ t: start('phone') + lock.notify, sound: 'notify' },
		{ t: start('phone') + lock.open, sound: 'tick' },
		{ t: at('phone', phone, find(chat, 'phone', (event) => event.type === 'tap').t, lock.length), sound: 'tap' },
		{ t: at('review', review, commit), sound: 'sparkle' },
		{ t: at('split', splitKeys(split), mark(split, 'desktop', 'asked')), sound: 'notify' },
		{ t: at('epic', epics, mark(epic, 'desktop', 'started')), sound: 'sparkle' },
		{ t: at('epic', epics, clicks(epic, mark(epic, 'desktop', 'land'), Infinity).at(-1)!), sound: 'tap' },
		{ t: at('fleet', fleets, find(fleet, 'desktop', (event) => event.type === 'keys').t), sound: 'whoosh' },
		{ t: start('themes'), sound: 'whoosh' }
	];
}
