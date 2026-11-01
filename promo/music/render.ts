import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { cues, scenes, type Timelines } from '../src/timing.ts';
import { bar, bpm, sections, start, total } from '../src/edit.ts';

const rate = 48_000;
const beat = 60 / bpm;
const root = fileURLToPath(new URL('..', import.meta.url));
const timelines = Object.fromEntries(scenes.map((scene) => [scene, JSON.parse(readFileSync(join(root, `public/footage/${scene}/timeline.json`), 'utf8'))])) as Timelines;
const output = join(root, 'public/music/promo.wav');
const frames = Math.ceil(total * rate);

class Bus {
	l = new Float32Array(frames);
	r = new Float32Array(frames);

	add(index: number, value: number, pan = 0) {
		if (index < 0 || index >= frames) return;
		this.l[index] += value * Math.cos((pan + 1) * Math.PI / 4);
		this.r[index] += value * Math.sin((pan + 1) * Math.PI / 4);
	}
}

const pad = new Bus();
const pluck = new Bus();
const bass = new Bus();
const drums = new Bus();
const reverbSend = new Bus();
const delaySend = new Bus();

let seed = 7;
function noise() {
	seed = (seed + 0x6d2b79f5) | 0;
	let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
	t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
	return ((t ^ (t >>> 14)) >>> 0) / 2147483648 - 1;
}

const hz = (note: number) => 440 * 2 ** ((note - 69) / 12);

function blep(phase: number, step: number) {
	if (phase < step) {
		const t = phase / step;
		return t + t - t * t - 1;
	}
	if (phase > 1 - step) {
		const t = (phase - 1) / step;
		return t * t + t + t + 1;
	}
	return 0;
}

class Filter {
	private a = 0;
	private b = 0;
	private g = 0;
	private k = 1;

	tune(cutoff: number, q = 0.707) {
		this.g = Math.tan(Math.PI * Math.min(cutoff, rate * 0.45) / rate);
		this.k = 1 / q;
	}

	run(x: number) {
		const a1 = 1 / (1 + this.g * (this.g + this.k));
		const v3 = x - this.b;
		const v1 = a1 * this.a + this.g * a1 * v3;
		const v2 = this.b + this.g * v1;
		this.a = 2 * v1 - this.a;
		this.b = 2 * v2 - this.b;
		return { low: v2, band: v1, high: x - this.k * v1 - v2 };
	}
}

function at(seconds: number) {
	return Math.round(seconds * rate);
}

function padNote(note: number, time: number, duration: number, level: number) {
	const attack = 0.7;
	const release = 1.4;
	const detunes = [-0.09, -0.03, 0.04, 0.1];
	const phases = detunes.map(() => noise() * 0.5 + 0.5);
	const filter = [new Filter(), new Filter()];
	const length = at(duration + release);
	const begin = at(time);
	for (let i = 0; i < length; i++) {
		const t = i / rate;
		const envelope = Math.min(1, t / attack) * (t > duration ? Math.exp(-(t - duration) / (release / 4)) : 1);
		if (i % 32 === 0) for (const side of filter) side.tune(700 + 500 * Math.sin(2 * Math.PI * 0.11 * (time + t)) ** 2, 0.8);
		let left = 0;
		let right = 0;
		detunes.forEach((detune, index) => {
			const step = hz(note + detune) / rate;
			phases[index] = (phases[index] + step) % 1;
			const saw = 2 * phases[index] - 1 - blep(phases[index], step);
			if (index % 2) right += saw;
			else left += saw;
		});
		pad.add(begin + i, filter[0].run(left).low * envelope * level, -0.6);
		pad.add(begin + i, filter[1].run(right).low * envelope * level, 0.6);
		reverbSend.add(begin + i, (left + right) * 0.15 * envelope * level);
	}
}

function pluckNote(note: number, time: number, level: number, pan: number) {
	const filter = new Filter();
	const length = at(0.6);
	const begin = at(time);
	let phase = 0;
	const step = hz(note) / rate;
	for (let i = 0; i < length; i++) {
		const t = i / rate;
		if (i % 16 === 0) filter.tune(500 + 4200 * Math.exp(-t / 0.07), 1.1);
		phase = (phase + step) % 1;
		const saw = 2 * phase - 1 - blep(phase, step);
		const square = (phase < 0.5 ? 1 : -1) * 0.4;
		const value = filter.run(saw + square).low * Math.exp(-t / 0.16) * Math.min(1, t / 0.002) * level;
		pluck.add(begin + i, value, pan);
		delaySend.add(begin + i, value * 0.5, pan);
		reverbSend.add(begin + i, value * 0.25);
	}
}

function bassNote(note: number, time: number, duration: number, level: number) {
	const filter = new Filter();
	filter.tune(320, 0.9);
	const length = at(duration + 0.05);
	const begin = at(time);
	let phase = 0;
	const step = hz(note) / rate;
	for (let i = 0; i < length; i++) {
		const t = i / rate;
		phase = (phase + step) % 1;
		const envelope = Math.min(1, t / 0.006) * (t > duration ? Math.exp(-(t - duration) / 0.012) : Math.exp(-t / 0.6) * 0.5 + 0.5);
		const value = (Math.sin(2 * Math.PI * phase) + 0.5 * filter.run(2 * phase - 1 - blep(phase, step)).low) * envelope * level;
		bass.add(begin + i, value);
	}
}

const kicks: number[] = [];

function kick(time: number, level: number) {
	kicks.push(time);
	const begin = at(time);
	let phase = 0;
	for (let i = 0; i < at(0.5); i++) {
		const t = i / rate;
		phase += (44 + 110 * Math.exp(-t / 0.028)) / rate;
		const click = t < 0.004 ? noise() * (1 - t / 0.004) * 0.3 : 0;
		drums.add(begin + i, (Math.sin(2 * Math.PI * phase) * Math.exp(-t / 0.26) + click) * level);
	}
}

function hat(time: number, level: number, pan: number) {
	const filter = new Filter();
	filter.tune(8000, 0.8);
	const begin = at(time);
	for (let i = 0; i < at(0.08); i++) {
		const t = i / rate;
		drums.add(begin + i, filter.run(noise()).high * Math.exp(-t / 0.022) * level, pan);
	}
}

function clap(time: number, level: number) {
	const filter = new Filter();
	filter.tune(1500, 1.4);
	const begin = at(time);
	for (let i = 0; i < at(0.35); i++) {
		const t = i / rate;
		const bursts = [0, 0.011, 0.022].reduce((sum, offset) => sum + (t >= offset && t < offset + 0.01 ? Math.exp(-(t - offset) / 0.004) : 0), 0);
		const value = filter.run(noise()).band * (bursts + (t > 0.022 ? Math.exp(-(t - 0.022) / 0.11) : 0)) * level;
		drums.add(begin + i, value);
		reverbSend.add(begin + i, value * 0.6);
	}
}

function riser(time: number, duration: number, level: number) {
	const filter = new Filter();
	const begin = at(time);
	for (let i = 0; i < at(duration); i++) {
		const progress = i / at(duration);
		if (i % 16 === 0) filter.tune(250 * 28 ** progress, 2.2);
		const value = filter.run(noise()).band * progress ** 2.2 * level;
		drums.add(begin + i, value, Math.sin(progress * 9) * 0.4);
		reverbSend.add(begin + i, value * 0.8);
	}
}

function impact(time: number, level: number) {
	const filter = new Filter();
	filter.tune(260, 0.7);
	const begin = at(time);
	let phase = 0;
	for (let i = 0; i < at(1.6); i++) {
		const t = i / rate;
		phase += (34 + 40 * Math.exp(-t / 0.08)) / rate;
		const value = (Math.sin(2 * Math.PI * phase) * 0.9 + filter.run(noise()).low * 0.7) * Math.exp(-t / 0.45) * level;
		drums.add(begin + i, value);
		reverbSend.add(begin + i, value * 0.5);
	}
}

function chime(note: number, time: number, level: number, pan: number) {
	const begin = at(time);
	const carrier = hz(note);
	for (let i = 0; i < at(2.4); i++) {
		const t = i / rate;
		const index = 2.2 * Math.exp(-t / 0.25);
		const value = Math.sin(2 * Math.PI * carrier * t + index * Math.sin(2 * Math.PI * carrier * 3.5 * t)) * Math.exp(-t / 0.7) * Math.min(1, t / 0.003) * level;
		pluck.add(begin + i, value, pan);
		reverbSend.add(begin + i, value * 0.9);
		delaySend.add(begin + i, value * 0.3, pan);
	}
}

const chords = {
	G: { bass: 31, pad: [55, 59, 62, 66, 69], arp: [67, 71, 74, 78] },
	A: { bass: 33, pad: [52, 57, 61, 64, 66], arp: [69, 73, 76, 78] },
	D: { bass: 38, pad: [50, 57, 61, 64, 66], arp: [69, 74, 76, 78] },
	Bm: { bass: 35, pad: [54, 59, 62, 66, 69], arp: [66, 69, 74, 76] }
};
function whoosh(time: number, duration: number, level: number) {
	const filter = new Filter();
	const begin = at(time);
	for (let i = 0; i < at(duration); i++) {
		const progress = i / at(duration);
		if (i % 16 === 0) filter.tune(6000 * 0.08 ** progress, 1.6);
		const value = filter.run(noise()).band * Math.sin(Math.PI * progress) ** 2 * level;
		drums.add(begin + i, value, progress - 0.5);
		reverbSend.add(begin + i, value * 0.5);
	}
}

const loop: (keyof typeof chords)[] = ['D', 'Bm', 'G', 'A'];
const pattern = [0, 2, 1, 3, 2, 0, 3, 1];
const plan = sections.flatMap((section, order) => Array.from({ length: section.bars }, (_, within) => ({ section, within, next: sections[order + 1] })));
const main = sections[0].bars;

plan.forEach(({ section, within, next }, index) => {
	const time = index * bar;
	const style = section.style;
	const name = style === 'intro' ? (['G', 'A'] as const)[within % 2] : style === 'outro' ? (['G', 'D'] as const)[within % 2] : loop[(index - main) % 4];
	const chord = chords[name];
	const last = index === plan.length - 1;
	const first = within === 0;
	const ending = within === section.bars - 1;
	for (const note of chord.pad) padNote(note, time, last ? bar * 0.6 : bar, style === 'outro' || style === 'break' ? 0.036 : 0.03);

	if (style === 'intro') {
		for (let step = 0; step < 8; step++) pluckNote(chord.arp[pattern[step]] + 12, time + step * beat / 2, 0.025 + 0.015 * index, step % 2 ? 0.35 : -0.35);
		if (ending) riser(time, bar, 0.06);
		return;
	}
	if (style === 'outro') {
		if (first) impact(time, 0.35);
		for (let step = 0; step < 4; step++) pluckNote(chord.arp[pattern[step]] + 12, time + step * beat, 0.04, step % 2 ? 0.4 : -0.4);
		if (last) [74, 81, 86].forEach((note, order) => chime(note, time + order * 0.09, 0.05, order - 1));
		return;
	}
	if (style === 'break') {
		bassNote(chord.bass + 12, time, bar * 0.95, 0.09);
		for (let step = 0; step < 8; step++) {
			const t = time + step * beat / 2;
			pluckNote(chord.arp[pattern[step]] + 12, t, 0.055, step % 2 ? 0.45 : -0.45);
			if (step % 2 === 1) hat(t, 0.05, 0.2);
		}
		if (ending && next) riser(time, bar, 0.07);
		return;
	}

	if (first) impact(time, 0.28);
	const lift = style === 'lift' || style === 'peak';
	for (let step = 0; step < 16; step++) {
		const t = time + step * beat / 4;
		if (step % 4 === 0) kick(t, 0.3);
		if (step % 4 === 2) hat(t, 0.11, 0.2);
		else if (lift && step % 2 === 1) hat(t, 0.05, -0.25);
		else if (style === 'peak') hat(t, 0.035, 0.3);
		if (lift && step % 8 === 4) clap(t, 0.15);
		const octave = style === 'peak' || (lift && step >= 8) ? 12 : 0;
		pluckNote(chord.arp[pattern[step % 8]] + octave, t, step % 4 === 0 ? 0.085 : 0.065, step % 2 ? 0.45 : -0.45);
		if (step % 2 === 0) bassNote(chord.bass + 12, t, beat / 2 * 0.8, 0.13);
	}
	if (style === 'peak') chime(chord.arp[3] + 12, time, 0.025, within % 2 ? 0.5 : -0.5);
	if (ending && next && next.style !== 'break' && next.style !== 'outro') riser(time + bar / 2, bar / 2, 0.08);
});

for (const cue of cues(timelines)) {
	if (cue.sound === 'notify') {
		chime(88, cue.t, 0.06, 0.2);
		chime(93, cue.t + 0.12, 0.05, 0.2);
	}
	if (cue.sound === 'tick') chime(86, cue.t, 0.025, 0.2);
	if (cue.sound === 'tap') [81, 86, 90].forEach((note, order) => chime(note, cue.t + order * 0.07, 0.07, order * 0.5 - 0.5));
	if (cue.sound === 'sparkle') [86, 93].forEach((note, order) => chime(note, cue.t + order * 0.05, 0.045, order - 0.5));
	if (cue.sound === 'whoosh') whoosh(cue.t - 0.18, 0.4, 0.05);
}
chime(74, 0.15, 0.04, 0);

function duck(time: number) {
	let gain = 1;
	for (const kickTime of kicks) {
		if (kickTime > time) break;
		gain = 1 - 0.55 * Math.exp(-(time - kickTime) / 0.13);
	}
	return gain;
}

function reverb(input: Bus) {
	const scale = rate / 44_100;
	const combs = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
	const allpasses = [556, 441, 341, 225];
	const out = new Bus();
	for (const [side, spread] of [[out.l, 0], [out.r, 23]] as const) {
		const mono = input.l.map((value, index) => (value + input.r[index]) * 0.5);
		const wet = new Float32Array(frames);
		for (const length of combs) {
			const buffer = new Float32Array(Math.round((length + spread) * scale));
			let position = 0;
			let store = 0;
			for (let i = 0; i < frames; i++) {
				const value = buffer[position];
				store = value * 0.7 + store * 0.3;
				buffer[position] = mono[i] * 0.015 + store * 0.86;
				position = (position + 1) % buffer.length;
				wet[i] += value;
			}
		}
		for (const length of allpasses) {
			const buffer = new Float32Array(Math.round((length + spread) * scale));
			let position = 0;
			for (let i = 0; i < frames; i++) {
				const delayed = buffer[position];
				buffer[position] = wet[i] + delayed * 0.5;
				wet[i] = delayed - wet[i];
				position = (position + 1) % buffer.length;
			}
		}
		side.set(wet);
	}
	return out;
}

function delay(input: Bus) {
	const length = at(beat * 0.75);
	const out = new Bus();
	const left = new Float32Array(length);
	const right = new Float32Array(length);
	const filter = [new Filter(), new Filter()];
	filter.forEach((side) => side.tune(3200));
	let position = 0;
	for (let i = 0; i < frames; i++) {
		const [l, r] = [left[position], right[position]];
		left[position] = input.l[i] + filter[0].run(r).low * 0.38;
		right[position] = input.r[i] * 0.4 + filter[1].run(l).low * 0.38;
		out.l[i] = l;
		out.r[i] = r;
		position = (position + 1) % length;
	}
	return out;
}

const space = reverb(reverbSend);
const echo = delay(delaySend);
const highpass = [new Filter(), new Filter()];
highpass.forEach((side) => side.tune(28));
const fade = start('outro') + bar * 1.4;
const mix = [new Float32Array(frames), new Float32Array(frames)];
let peak = 0;
for (let i = 0; i < frames; i++) {
	const time = i / rate;
	const gain = duck(time);
	const ending = time > fade ? Math.cos(Math.min(1, (time - fade) / (total - fade)) * Math.PI / 2) : 1;
	for (const [channel, key] of [[0, 'l'], [1, 'r']] as const) {
		const value = (pad[key][i] * gain + pluck[key][i] * (0.6 + 0.4 * gain) + bass[key][i] * gain + drums[key][i] + space[key][i] * 0.9 + echo[key][i] * 0.5) * ending;
		mix[channel][i] = Math.tanh(highpass[channel].run(value).high * 1.3);
		peak = Math.max(peak, Math.abs(mix[channel][i]));
	}
}

const pcm = Buffer.alloc(44 + frames * 4);
pcm.write('RIFF', 0);
pcm.writeUInt32LE(36 + frames * 4, 4);
pcm.write('WAVEfmt ', 8);
pcm.writeUInt32LE(16, 16);
pcm.writeUInt16LE(1, 20);
pcm.writeUInt16LE(2, 22);
pcm.writeUInt32LE(rate, 24);
pcm.writeUInt32LE(rate * 4, 28);
pcm.writeUInt16LE(4, 32);
pcm.writeUInt16LE(16, 34);
pcm.write('data', 36);
pcm.writeUInt32LE(frames * 4, 40);
const normal = 0.89 / peak;
for (let i = 0; i < frames; i++) {
	for (const channel of [0, 1]) {
		const dither = (Math.random() - Math.random()) / 32768;
		pcm.writeInt16LE(Math.max(-32768, Math.min(32767, Math.round((mix[channel][i] * normal + dither) * 32767))), 44 + i * 4 + channel * 2);
	}
}
mkdirSync(dirname(output), { recursive: true });
writeFileSync(output, pcm);
console.log(`${output}: ${total.toFixed(2)}s, peak before normalizing ${peak.toFixed(3)}`);
