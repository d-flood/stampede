import { AbsoluteFill, interpolate } from 'remotion';
import { length, mark, type Event } from '../edit.ts';
import type { Frames } from '../footage.tsx';
import { colors, text } from '../parts.tsx';
import { Lower, Screen, focus, frame, on, seconds, useClock, zoom } from '../Screen.tsx';
import { rigsKeys } from './rigs.ts';
import timeline from '../../public/footage/rigs/timeline.json';
import desktop from '../../public/footage/rigs/desktop/frames.json';

const rigs = timeline as Event[];
const keys = rigsKeys(rigs);
const events = on(rigs, 'desktop');
const editor = focus(rigs, 'editor');
const menu = focus(rigs, 'menu');
const hint = focus(rigs, 'hint');
const chips = focus(rigs, 'chips');
const codex = mark(rigs, 'desktop', 'codex');
const closed = mark(rigs, 'desktop', 'closed');
const leader = mark(rigs, 'desktop', 'leader');
const launched = mark(rigs, 'desktop', 'launched');
const presses = events.filter((event) => event.type === 'press');
const shots = [
	zoom(mark(rigs, 'desktop', 'start'), 1),
	zoom(editor.t + 0.2, 1.55, editor),
	zoom(closed - 0.3, 1.55, editor),
	zoom(closed + 0.2, 1),
	zoom(menu.t + 0.1, 1.5, menu),
	zoom(leader - 0.1, 1.5, menu),
	zoom(leader + 0.3, 1),
	zoom(hint.t, 1.4, hint),
	zoom(launched - 0.1, 1.4, hint),
	zoom(launched + 0.5, 1.5, chips)
];

// Leader keys are a sequence, not a chord, so each press adds a keycap instead of replacing the last.
const Presses = ({ t }: { t: number }) => {
	const shown = presses.filter((press) => t >= press.t);
	if (!shown.length) return null;
	const opacity = interpolate(t, [shown[0].t, shown[0].t + 0.15, presses.at(-1)!.t + 1.2, presses.at(-1)!.t + 1.5], [0, 1, 1, 0], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' });
	const scale = frame.width / 1280;
	return (
		<div style={{ position: 'absolute', right: 1920 - frame.left - frame.width + 32 * scale, top: frame.top + 40 + (800 - 32) * scale, transform: 'translateY(-100%)', display: 'flex', alignItems: 'center', gap: 12, opacity }}>
			{shown.map((press, index) => (
				<div key={press.t} style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
					{index > 0 && <span style={{ color: colors.muted, fontFamily: text, fontSize: 30 }}>›</span>}
					{press.name!.split(' + ').map((key) => (
						<div key={key} style={{ minWidth: 56, padding: '10px 16px', borderRadius: 12, background: 'rgba(20, 18, 16, 0.92)', border: '1px solid rgba(255,255,255,0.18)', boxShadow: '0 6px 0 rgba(0,0,0,0.5)', color: colors.ink, fontFamily: text, fontWeight: 600, fontSize: 28, textAlign: 'center' }}>{key}</div>
					))}
				</div>
			))}
		</div>
	);
};

export const Rigs = () => {
	const { t, at } = useClock(keys);
	return (
		<AbsoluteFill>
			<Screen scene="rigs" frames={desktop as Frames} events={events} t={t} shots={shots} rest={{ x: 1150, y: 300 }} />
			<Presses t={t} />
			<Lower from={0} to={at(codex)}>A Rig is an agent, configured and ready.</Lower>
			<Lower from={at(codex)} to={at(closed)}>Provider, model, effort, permissions.</Lower>
			<Lower from={at(closed)} to={at(leader)}>Launch one with a click…</Lower>
			<Lower from={at(leader)} to={seconds(length('rigs'))}>…or a keystroke.</Lower>
		</AbsoluteFill>
	);
};
