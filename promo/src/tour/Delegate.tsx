import { AbsoluteFill } from 'remotion';
import { find, length, mark, type Event } from '../edit.ts';
import type { Frames } from '../footage.tsx';
import { Lower, Screen, focus, on, seconds, useClock, zoom } from '../Screen.tsx';
import { delegateKeys } from './delegate.ts';
import timeline from '../../public/footage/delegate/timeline.json';
import frames from '../../public/footage/delegate/desktop/frames.json';

const events = timeline as Event[];
const keys = delegateKeys(events);
const typed = find(events, 'desktop', (event) => event.type === 'type');
const sent = mark(events, 'desktop', 'sent');
const dock = focus(events, 'dock');
const opened = mark(events, 'desktop', 'opened');
const header = focus(events, 'header');
const report = focus(events, 'report');
const back = mark(events, 'desktop', 'back');
const shots = [
	zoom(mark(events, 'desktop', 'start'), 1),
	zoom(typed.t + 0.5, 1.3, typed),
	zoom(sent + 0.3, 1.3, typed),
	zoom(dock.t, 1.2, dock),
	zoom(opened - 0.1, 1.2, dock),
	zoom(opened + 0.6, 1.2, header),
	zoom(report.t - 0.3, 1.2, header),
	zoom(report.t + 0.4, 1.3, report),
	zoom(back - 0.4, 1.3, report),
	zoom(back + 0.5, 1)
];

export const Delegate = () => {
	const { t, at } = useClock(keys);
	return (
		<AbsoluteFill>
			<Screen scene="delegate" frames={frames as Frames} events={on(events, 'desktop')} t={t} shots={shots} rest={{ x: 900, y: 300 }} />
			<Lower from={0} to={at(dock.t)}>Claude can hand work to Codex. Or any Rig.</Lower>
			<Lower from={at(dock.t)} to={at(report.t)}>Follow the subagent live.</Lower>
			<Lower from={at(report.t)} to={seconds(length('delegate'))}>The report comes back to Claude.</Lower>
		</AbsoluteFill>
	);
};
