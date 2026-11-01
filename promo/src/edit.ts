export type Box = { x: number; y: number; width: number; height: number };
export type Event = { t: number; device: string; type: string; x?: number; y?: number; name?: string; box?: Box };
export type Key = { out: number; foot: number };

export const fps = 30;
export const bpm = 100;
export const bar = 240 / bpm;

export const sections = [
	{ name: 'intro', bars: 2, style: 'intro' },
	{ name: 'chat', bars: 10, style: 'groove' },
	{ name: 'phone', bars: 4, style: 'lift' },
	{ name: 'review', bars: 4, style: 'break' },
	{ name: 'split', bars: 2, style: 'groove' },
	{ name: 'epic', bars: 8, style: 'peak' },
	{ name: 'fleet', bars: 4, style: 'lift' },
	{ name: 'themes', bars: 2, style: 'peak' },
	{ name: 'outro', bars: 2, style: 'outro' }
] as const;

export type Section = (typeof sections)[number]['name'];

export function start(name: Section) {
	let bars = 0;
	for (const section of sections) {
		if (section.name === name) return bars * bar;
		bars += section.bars;
	}
	throw new Error(name);
}

export function length(name: Section) {
	return sections.find((section) => section.name === name)!.bars * bar;
}

export const total = sections.reduce((sum, section) => sum + section.bars, 0) * bar;

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
