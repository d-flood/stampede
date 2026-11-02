import { AbsoluteFill } from 'remotion';
import { length, mark, type Event } from '../edit.ts';
import type { Frames } from '../footage.tsx';
import { Lower, Screen, focus, on, seconds, useClock, zoom } from '../Screen.tsx';
import { worktreesKeys } from './worktrees.ts';
import timeline from '../../public/footage/worktrees/timeline.json';
import desktop from '../../public/footage/worktrees/desktop/frames.json';

const worktrees = timeline as Event[];
const keys = worktreesKeys(worktrees);
const events = on(worktrees, 'desktop');
const menu = focus(worktrees, 'menu');
const dialog = focus(worktrees, 'dialog');
const rail = focus(worktrees, 'rail');
const created = mark(worktrees, 'desktop', 'created');
const shots = [
	zoom(mark(worktrees, 'desktop', 'start'), 1),
	zoom(menu.t, 1.4, menu),
	zoom(dialog.t, 1.4, menu),
	zoom(dialog.t + 0.4, 1.35, dialog),
	zoom(created, 1.35, dialog),
	zoom(rail.t + 0.4, 1.5, rail),
	zoom(rail.t + 1.2, 1.5, rail),
	zoom(rail.t + 1.8, 1)
];

export const Worktrees = () => {
	const { t, at } = useClock(keys);
	return (
		<AbsoluteFill>
			<Screen scene="worktrees" frames={desktop as Frames} events={events} t={t} shots={shots} rest={{ x: 700, y: 420 }} />
			<Lower from={0} to={at(created + 0.6)}>Try an idea in its own worktree.</Lower>
			<Lower from={at(created + 0.6)} to={seconds(length('worktrees'))}>Each one is a Project, with its own agents.</Lower>
		</AbsoluteFill>
	);
};
