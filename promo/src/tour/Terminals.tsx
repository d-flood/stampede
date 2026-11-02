import { AbsoluteFill } from 'remotion';
import { find, length, mark, type Event } from '../edit.ts';
import type { Frames } from '../footage.tsx';
import { Lower, Screen, focus, on, seconds, useClock, zoom } from '../Screen.tsx';
import { terminalsKeys } from './terminals.ts';
import timeline from '../../public/footage/terminals/timeline.json';
import frames from '../../public/footage/terminals/desktop/frames.json';

const events = timeline as Event[];
const keys = terminalsKeys(events);
const typed = find(events, 'desktop', (event) => event.type === 'type');
const sent = mark(events, 'desktop', 'sent');
const staged = mark(events, 'desktop', 'staged');
const ran = mark(events, 'desktop', 'ran');
const output = focus(events, 'output');
const shots = [
	zoom(mark(events, 'desktop', 'start'), 1),
	zoom(typed.t + 0.4, 1.3, typed),
	zoom(sent + 0.3, 1.3, typed),
	zoom(sent + 1.0, 1),
	zoom(ran + 0.1, 1),
	zoom(output.t + 0.4, 1.6, output)
];

export const Terminals = () => {
	const { t, at } = useClock(keys);
	return (
		<AbsoluteFill>
			<Screen scene="terminals" frames={frames as Frames} events={on(events, 'desktop')} t={t} shots={shots} rest={{ x: 900, y: 300 }} />
			<Lower from={0} to={at(staged)}>Agents stage commands that need you.</Lower>
			<Lower from={at(staged)} to={seconds(length('terminals'))}>Review it, edit it, run it.</Lower>
		</AbsoluteFill>
	);
};
