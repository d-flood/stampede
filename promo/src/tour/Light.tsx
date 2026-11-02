import { AbsoluteFill, interpolate } from 'remotion';
import { length, mark, type Event } from '../edit.ts';
import type { Frames } from '../footage.tsx';
import { colors, display, text } from '../parts.tsx';
import { Lower, Screen, focus, frame, on, seconds, useClock, zoom } from '../Screen.tsx';
import { lightKeys } from './light.ts';
import timeline from '../../public/footage/light/timeline.json';
import desktop from '../../public/footage/light/desktop/frames.json';
import stats from '../../public/footage/light/stats.json';

type Stat = { t: number; server: number; terminals: number };

const light = timeline as Event[];
const samples = stats as Stat[];
const keys = lightKeys(light);
const events = on(light, 'desktop');
const hops = events.filter((event) => event.type === 'click');
const lsp = focus(light, 'lsp');
const peak = Math.round(Math.max(...samples.map((sample) => sample.server)));
const shots = [
	zoom(mark(light, 'desktop', 'start'), 1),
	zoom(lsp.t - 0.2, 1),
	zoom(lsp.t + 0.6, 1.6, lsp),
	zoom(mark(light, 'desktop', 'end'), 1.6, lsp)
];

const Meter = ({ t }: { t: number }) => {
	const now = samples.filter((sample) => sample.t <= t).at(-1) ?? samples[0];
	const opacity = interpolate(t, [samples[0].t, samples[0].t + 0.4], [0, 1], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' });
	return (
		<div style={{ position: 'absolute', left: frame.left + frame.width + 28, top: 300, width: 1920 - frame.left - frame.width - 40, color: colors.ink, opacity }}>
			<div style={{ fontFamily: text, fontWeight: 600, fontSize: 15, letterSpacing: '0.14em', textTransform: 'uppercase', color: colors.ember }}>Measured live</div>
			<div style={{ marginTop: 22, fontFamily: text, fontSize: 19, color: colors.muted }}>Stampede server</div>
			<div style={{ fontFamily: display, fontWeight: 700, fontSize: 58, letterSpacing: '-0.02em', fontVariantNumeric: 'tabular-nums' }}>{now.server.toFixed(1)}<span style={{ fontSize: 26, marginLeft: 6, color: colors.muted }}>MiB</span></div>
			<div style={{ marginTop: 18, fontFamily: text, fontSize: 19, lineHeight: 1.5, color: colors.muted }}>4 projects<br />4 agents working<br />{now.terminals} terminals</div>
		</div>
	);
};

export const Light = () => {
	const { t, at } = useClock(keys);
	return (
		<AbsoluteFill>
			<Screen scene="light" frames={desktop as Frames} events={events} t={t} shots={shots} rest={{ x: 700, y: 420 }} />
			<Meter t={t} />
			<Lower from={0} to={at(hops[2].t)}>No Electron. One small Rust server.</Lower>
			<Lower from={at(hops[2].t)} to={at(mark(light, 'desktop', 'lsp'))}>{peak} MiB, with all of this running.</Lower>
			<Lower from={at(mark(light, 'desktop', 'lsp'))} to={seconds(length('light'))}>Language servers start only when you turn them on.</Lower>
		</AbsoluteFill>
	);
};
