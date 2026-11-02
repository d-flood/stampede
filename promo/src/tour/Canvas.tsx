import { AbsoluteFill } from 'remotion';
import { length, mark, type Event } from '../edit.ts';
import type { Frames } from '../footage.tsx';
import { Lower, Screen, focus, on, seconds, useClock, zoom } from '../Screen.tsx';
import { canvasKeys } from './canvas.ts';
import canvasTimeline from '../../public/footage/canvas/timeline.json';
import canvasDesktop from '../../public/footage/canvas/desktop/frames.json';

const timeline = canvasTimeline as Event[];
const keys = canvasKeys(timeline);
const events = on(timeline, 'desktop');
const card = focus(timeline, 'card');
const opened = mark(timeline, 'desktop', 'opened');
const versions = focus(timeline, 'versions');
const shots = [
	zoom(mark(timeline, 'desktop', 'start'), 1),
	zoom(card.t - 0.2, 1),
	zoom(card.t + 0.5, 1.3, card),
	zoom(opened - 0.1, 1.3, card),
	zoom(opened + 0.7, 1),
	zoom(versions.t, 1),
	zoom(versions.t + 0.6, 1.2, versions)
];

export const Canvas = () => {
	const { t, at } = useClock(keys);
	return (
		<AbsoluteFill>
			<Screen scene="canvas" frames={canvasDesktop as Frames} events={events} t={t} shots={shots} rest={{ x: 1000, y: 300 }} />
			<Lower from={0} to={at(opened)}>Mockups open on a live canvas.</Lower>
			<Lower from={at(opened)} to={at(versions.t)}>It updates as the agent works.</Lower>
			<Lower from={at(versions.t)} to={seconds(length('canvas'))}>Every revision is kept.</Lower>
		</AbsoluteFill>
	);
};
