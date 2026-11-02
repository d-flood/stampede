import { AbsoluteFill } from 'remotion';
import { length, mark, type Event } from '../edit.ts';
import type { Frames } from '../footage.tsx';
import { Lower, Screen, focus, on, seconds, useClock, zoom } from '../Screen.tsx';
import { grillKeys } from './grill.ts';
import timeline from '../../public/footage/grill/timeline.json';
import desktop from '../../public/footage/grill/desktop/frames.json';

const grill = timeline as Event[];
const keys = grillKeys(grill);
const events = on(grill, 'desktop');
const commands = focus(grill, 'commands');
const typed = events.filter((event) => event.type === 'type')[1];
const question = focus(grill, 'question');
const card = focus(grill, 'card');
const tickets = focus(grill, 'tickets');
const setup = focus(grill, 'setup');
const run = focus(grill, 'run');
const sent = mark(grill, 'desktop', 'sent');
const answered = mark(grill, 'desktop', 'answered');
const spec = mark(grill, 'desktop', 'spec');
const specified = mark(grill, 'desktop', 'specified');
const ticketing = mark(grill, 'desktop', 'ticketing');
const confirmed = mark(grill, 'desktop', 'confirmed');
const shots = [
	zoom(mark(grill, 'desktop', 'start'), 1),
	zoom(commands.t + 0.3, 1.25, commands),
	zoom(typed.t - 0.2, 1.25, commands),
	zoom(typed.t + 0.4, 1.3, typed),
	zoom(sent, 1.3, typed),
	zoom(sent + 0.8, 1),
	zoom(question.t, 1),
	zoom(question.t + 0.5, 1.3, question),
	zoom(answered, 1.3, question),
	zoom(answered + 0.8, 1),
	zoom(card.t - 0.1, 1),
	zoom(card.t + 0.5, 1.45, card),
	zoom(specified + 0.4, 1.45, card),
	zoom(specified + 1.2, 1),
	zoom(tickets.t, 1),
	zoom(tickets.t + 0.6, 1.2, tickets),
	zoom(setup.t - 0.2, 1.2, tickets),
	zoom(setup.t + 0.4, 1.25, setup),
	zoom(confirmed, 1.25, setup),
	zoom(run.t + 0.3, 1.35, run)
];

export const Grill = () => {
	const { t, at } = useClock(keys);
	return (
		<AbsoluteFill>
			<Screen scene="grill" frames={desktop as Frames} events={events} t={t} shots={shots} rest={{ x: 640, y: 400 }} />
			<Lower from={0} to={at(spec)}>/grill interviews you about the plan.</Lower>
			<Lower from={at(spec)} to={at(specified)}>/spec writes the Brief and creates the Epic.</Lower>
			<Lower from={at(specified)} to={at(ticketing)}>Open the Epic beside the Chat.</Lower>
			<Lower from={at(ticketing)} to={at(setup.t)}>/tickets slices it into Tickets with Blockers.</Lower>
			<Lower from={at(setup.t)} to={seconds(length('grill'))}>Confirm the Setup Command, and it is ready to Run.</Lower>
		</AbsoluteFill>
	);
};
