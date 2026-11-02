import { AbsoluteFill } from 'remotion';
import { find, length, mark, type Event } from '../edit.ts';
import type { Frames } from '../footage.tsx';
import { Lower, Screen, focus, on, seconds, useClock, zoom } from '../Screen.tsx';
import { gitKeys } from './git.ts';
import gitTimeline from '../../public/footage/git/timeline.json';
import gitDesktop from '../../public/footage/git/desktop/frames.json';

const git = gitTimeline as Event[];
const keys = gitKeys(git);
const events = on(git, 'desktop');
const sidebar = focus(git, 'sidebar');
const changes = { ...sidebar, box: { ...sidebar.box!, height: 560 } };
const picker = focus(git, 'picker');
const create = find(git, 'desktop', (event) => event.type === 'click' && event.t > picker.t).t;
const branched = mark(git, 'desktop', 'branched');
const committed = mark(git, 'desktop', 'committed');
const shots = [
	zoom(mark(git, 'desktop', 'start'), 1),
	zoom(picker.t - 0.3, 1),
	zoom(picker.t + 0.3, 1.6, picker),
	zoom(create, 1.6, picker),
	zoom(create + 0.6, 1),
	zoom(branched + 0.3, 1),
	zoom(branched + 1, 1.4, changes)
];

export const Committing = () => {
	const { t, at } = useClock(keys);
	return (
		<AbsoluteFill>
			<Screen scene="git" frames={gitDesktop as Frames} events={events} t={t} shots={shots} rest={{ x: 900, y: 500 }} />
			<Lower from={0} to={at(branched)}>Create a branch from the picker.</Lower>
			<Lower from={at(branched)} to={at(committed)}>Stage and commit with LazyGit keys.</Lower>
			<Lower from={at(committed)} to={seconds(length('git'))}>Stash, publish and read the history.</Lower>
		</AbsoluteFill>
	);
};
