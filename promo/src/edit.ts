export type Box = { x: number; y: number; width: number; height: number };
export type Event = { t: number; device: string; type: string; x?: number; y?: number; name?: string; box?: Box };
export type Key = { out: number; foot: number };

export const fps = 30;
export const bpm = 100;
export const bar = 240 / bpm;

export type Style = 'intro' | 'groove' | 'lift' | 'break' | 'peak' | 'outro';

export const sections = {
	intro: { bars: 2, style: 'intro' },
	chat: { bars: 10, style: 'groove' },
	phone: { bars: 4, style: 'lift' },
	review: { bars: 4, style: 'break' },
	split: { bars: 2, style: 'groove' },
	epic: { bars: 8, style: 'peak' },
	fleet: { bars: 4, style: 'lift' },
	themes: { bars: 2, style: 'peak' },
	outro: { bars: 2, style: 'outro' },
	rigs: { bars: 5, style: 'groove' },
	delegate: { bars: 6, style: 'lift' },
	media: { bars: 4, style: 'break' },
	canvas: { bars: 7, style: 'groove' },
	grill: { bars: 12, style: 'lift' },
	worktrees: { bars: 4, style: 'groove' },
	editor: { bars: 6, style: 'groove' },
	git: { bars: 6, style: 'lift' },
	terminals: { bars: 4, style: 'groove' },
	light: { bars: 4, style: 'break' },
	extras: { bars: 8, style: 'peak' }
} satisfies Record<string, { bars: number; style: Style }>;

export type Section = keyof typeof sections;

export const cuts: Record<'Promo' | 'Tour', Section[]> = {
	Promo: ['intro', 'chat', 'phone', 'review', 'split', 'epic', 'fleet', 'themes', 'outro'],
	Tour: ['intro', 'rigs', 'chat', 'phone', 'split', 'delegate', 'media', 'canvas', 'grill', 'epic', 'worktrees', 'editor', 'review', 'git', 'terminals', 'fleet', 'light', 'extras', 'themes', 'outro']
};

export type Cut = keyof typeof cuts;

export function start(cut: Cut, name: Section) {
	const index = cuts[cut].indexOf(name);
	if (index < 0) throw new Error(name);
	return cuts[cut].slice(0, index).reduce((sum, section) => sum + sections[section].bars, 0) * bar;
}

export const length = (name: Section) => sections[name].bars * bar;

export const total = (cut: Cut) => cuts[cut].reduce((sum, name) => sum + sections[name].bars, 0) * bar;

export function find(timeline: Event[], device: string, match: (event: Event) => boolean) {
	const event = timeline.find((event) => event.device === device && match(event));
	if (!event) throw new Error(`No matching ${device} event`);
	return event;
}

export function mark(timeline: Event[], device: string, name: string) {
	return find(timeline, device, (event) => event.name === name).t;
}

// Plays fixed spans at full speed and stretches the rest to fill the section.
export function retime(anchors: { t: number; fixed: boolean }[], duration: number): Key[] {
	const spans = anchors.slice(1).map((anchor, index) => ({ from: anchors[index].t, to: anchor.t, fixed: anchors[index].fixed }));
	const fixed = spans.filter((span) => span.fixed).reduce((sum, span) => sum + span.to - span.from, 0);
	const elastic = spans.filter((span) => !span.fixed).reduce((sum, span) => sum + span.to - span.from, 0);
	const rate = elastic / Math.max(0.001, duration - fixed);
	const keys: Key[] = [{ out: 0, foot: anchors[0].t }];
	for (const span of spans) {
		const out = keys.at(-1)!.out + (span.to - span.from) / (span.fixed ? 1 : rate);
		keys.push({ out, foot: span.to });
	}
	return keys;
}

function convert(keys: Key[], value: number, from: keyof Key, to: keyof Key) {
	let index = 0;
	while (index < keys.length - 2 && keys[index + 1][from] <= value) index++;
	const [a, b] = [keys[index], keys[index + 1] ?? keys[index]];
	if (b[from] === a[from]) return a[to] + (value - a[from]);
	return a[to] + ((value - a[from]) / (b[from] - a[from])) * (b[to] - a[to]);
}

export const footageAt = (keys: Key[], out: number) => convert(keys, out, 'out', 'foot');
export const outputAt = (keys: Key[], foot: number) => convert(keys, foot, 'foot', 'out');
