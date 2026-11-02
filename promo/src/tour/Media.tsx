import { AbsoluteFill } from 'remotion';
import { length, mark, type Event } from '../edit.ts';
import type { Frames } from '../footage.tsx';
import { Lower, Screen, focus, on, seconds, useClock, zoom } from '../Screen.tsx';
import { mediaKeys } from './media.ts';
import mediaTimeline from '../../public/footage/media/timeline.json';
import mediaDesktop from '../../public/footage/media/desktop/frames.json';

const timeline = mediaTimeline as Event[];
const keys = mediaKeys(timeline);
const events = on(timeline, 'desktop');
const image = focus(timeline, 'image');
const video = focus(timeline, 'video');
const shots = [
	zoom(mark(timeline, 'desktop', 'start'), 1),
	zoom(image.t, 1),
	zoom(image.t + 0.8, 1.3, image),
	zoom(video.t, 1.3, image),
	zoom(video.t + 0.3, 1.3, video)
];

export const Media = () => {
	const { t, at } = useClock(keys);
	return (
		<AbsoluteFill>
			<Screen scene="media" frames={mediaDesktop as Frames} events={events} t={t} shots={shots} rest={{ x: 1000, y: 300 }} />
			<Lower from={0} to={at(video.t)}>Agents show their work: screenshots,</Lower>
			<Lower from={at(video.t)} to={seconds(length('media'))}>and screen recordings, right in the chat.</Lower>
		</AbsoluteFill>
	);
};
