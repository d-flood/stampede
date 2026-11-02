import { AbsoluteFill, Sequence } from 'remotion';
import { bar, length, mark, type Event } from '../edit.ts';
import type { Frames } from '../footage.tsx';
import { Chip, Lower, Screen, focus, on, seconds, useClock, zoom } from '../Screen.tsx';
import { beats, extrasKeys, offset } from './extras.ts';
import extrasTimeline from '../../public/footage/extras/timeline.json';
import extrasDesktop from '../../public/footage/extras/desktop/frames.json';

const extras = extrasTimeline as Event[];
const keys = extrasKeys(extras);
const events = on(extras, 'desktop');
const at = (name: string) => mark(extras, 'desktop', name);
const shots = [
	zoom(at('clips'), 1.5, focus(extras, 'clips')),
	zoom(at('clips-end'), 1.5, focus(extras, 'clips')),
	zoom(at('queue'), 1.3, focus(extras, 'clips')),
	zoom(focus(extras, 'queue').t - 0.1, 1.3, focus(extras, 'clips')),
	zoom(focus(extras, 'queue').t + 0.5, 1.3, focus(extras, 'queue')),
	zoom(at('queue-end'), 1.3, focus(extras, 'queue')),
	zoom(at('rewind'), 1.1, focus(extras, 'rewind')),
	zoom(focus(extras, 'rewind').t + 0.4, 1.35, focus(extras, 'rewind')),
	zoom(at('rewind-end'), 1.35, focus(extras, 'rewind')),
	zoom(at('handoff'), 1),
	zoom(focus(extras, 'handoff').t, 1),
	zoom(focus(extras, 'handoff').t + 0.5, 1.3, focus(extras, 'handoff')),
	zoom(focus(extras, 'summary').t - 0.3, 1.3, focus(extras, 'handoff')),
	zoom(focus(extras, 'summary').t + 0.5, 1.3, focus(extras, 'summary')),
	zoom(at('handoff-end'), 1.3, focus(extras, 'summary')),
	zoom(at('search'), 1),
	zoom(focus(extras, 'search').t, 1.25, focus(extras, 'search')),
	zoom(at('search-end'), 1.25, focus(extras, 'search')),
	zoom(at('browser'), 1.1, focus(extras, 'browser')),
	zoom(at('browser-end'), 1.1, focus(extras, 'browser')),
	zoom(at('usage'), 1.6, focus(extras, 'usage')),
	zoom(at('usage-end'), 1.6, focus(extras, 'usage'))
];

export const Extras = () => {
	const { t } = useClock(keys);
	return (
		<AbsoluteFill>
			<Screen scene="extras" frames={extrasDesktop as Frames} events={events} t={t} shots={shots} rest={{ x: 1270, y: 790 }} />
			{beats.map((beat, index) => (
				<Sequence key={beat.name} from={seconds(offset(index))} durationInFrames={seconds(beat.bars * bar)}>
					<Chip>{beat.label}</Chip>
				</Sequence>
			))}
			<Lower from={0} to={seconds(offset(3))}>And the small things you reach for daily.</Lower>
			<Lower from={seconds(offset(3))} to={seconds(length('extras'))}>Switch agents mid-task. Search. Preview. Watch your limits.</Lower>
		</AbsoluteFill>
	);
};
