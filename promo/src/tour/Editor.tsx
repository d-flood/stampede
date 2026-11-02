import { AbsoluteFill } from 'remotion';
import { find, length, mark, type Event } from '../edit.ts';
import type { Frames } from '../footage.tsx';
import { Lower, Screen, focus, on, seconds, useClock, zoom } from '../Screen.tsx';
import { editorKeys } from './editor.ts';
import editorTimeline from '../../public/footage/editor/timeline.json';
import editorDesktop from '../../public/footage/editor/desktop/frames.json';

const editor = editorTimeline as Event[];
const keys = editorKeys(editor);
const events = on(editor, 'desktop');
const code = focus(editor, 'code');
const tooltip = focus(editor, 'tooltip');
const completion = focus(editor, 'completion');
const started = mark(editor, 'desktop', 'start');
const typed = mark(editor, 'desktop', 'typed');
const fixed = mark(editor, 'desktop', 'fixed');
const save = find(editor, 'desktop', (event) => event.type === 'keys' && event.name === 'Ctrl + S').t;
const shots = [
	zoom(started, 1),
	zoom(started + 0.5, 1),
	zoom(started + 1.3, 1.4, code),
	zoom(tooltip.t - 0.4, 1.4, code),
	zoom(tooltip.t + 0.3, 1.7, tooltip),
	zoom(completion.t - 0.6, 1.7, tooltip),
	zoom(completion.t + 0.1, 1.6, completion),
	zoom(fixed, 1.6, completion),
	zoom(fixed + 0.6, 1.4, code),
	zoom(save + 0.5, 1.4, code),
	zoom(save + 1.3, 1)
];

export const Editing = () => {
	const { t, at } = useClock(keys);
	return (
		<AbsoluteFill>
			<Screen scene="editor" frames={editorDesktop as Frames} events={events} t={t} shots={shots} rest={{ x: 1000, y: 600 }} />
			<Lower from={0} to={at(typed)}>A real editor, with change markers.</Lower>
			<Lower from={at(typed)} to={at(tooltip.t - 0.4)}>Language servers run only when you switch them on.</Lower>
			<Lower from={at(tooltip.t - 0.4)} to={seconds(length('editor'))}>Errors, types and completion.</Lower>
		</AbsoluteFill>
	);
};
