import type { ReactNode } from 'react';
import { AbsoluteFill, Audio, Img, Sequence, interpolate, spring, staticFile, useCurrentFrame, useVideoConfig } from 'remotion';
import { bar, cuts, find, footageAt, fps, length, mark, outputAt, start, type Cut, type Event, type Section } from './edit.ts';
import { Footage, Ripples, centre, ease, type Frames } from './footage.tsx';
import { Backdrop, Caption, LockScreen, Logo, Phone, colors, display, text, useFade } from './parts.tsx';
import { Chip, Lower, Screen, Window, focus, on, seconds, useClock, zoom } from './Screen.tsx';
import { chatKeys, epicKeys, fleetKeys, lock, phoneKeys, reviewKeys, splitKeys } from './timing.ts';
import chatTimeline from '../public/footage/chat/timeline.json';
import chatDesktop from '../public/footage/chat/desktop/frames.json';
import chatPhone from '../public/footage/chat/phone/frames.json';
import splitTimeline from '../public/footage/split/timeline.json';
import splitDesktop from '../public/footage/split/desktop/frames.json';
import epicTimeline from '../public/footage/epic/timeline.json';
import epicDesktop from '../public/footage/epic/desktop/frames.json';
import fleetTimeline from '../public/footage/fleet/timeline.json';
import fleetDesktop from '../public/footage/fleet/desktop/frames.json';
import themeStills from '../public/footage/themes/desktop/frames.json';

const chat = chatTimeline as Event[];
const split = splitTimeline as Event[];
const epic = epicTimeline as Event[];
const fleet = fleetTimeline as Event[];

const Part = ({ cut, name, children }: { cut: Cut; name: Section; children: ReactNode }) => (
	<Sequence from={seconds(start(cut, name))} durationInFrames={seconds(length(name))}>
		<Faded duration={seconds(length(name))}>{children}</Faded>
	</Sequence>
);

const Faded = ({ duration, children }: { duration: number; children: ReactNode }) => <AbsoluteFill style={{ opacity: useFade(duration) }}>{children}</AbsoluteFill>;

export const Title = ({ tagline, footer }: { tagline: string; footer?: string }) => {
	const now = useCurrentFrame();
	const { fps } = useVideoConfig();
	const rise = spring({ frame: now, fps, config: { damping: 14, mass: 0.8 } });
	const late = spring({ frame: now - 10, fps, config: { damping: 20 } });
	return (
		<AbsoluteFill style={{ alignItems: 'center', justifyContent: 'center', gap: 18 }}>
			<div style={{ display: 'flex', alignItems: 'center', gap: 28, transform: `translateY(${(1 - rise) * 40}px)`, opacity: rise }}>
				<Logo size={150} />
				<div style={{ fontFamily: display, fontWeight: 800, fontSize: 150, color: colors.ink, letterSpacing: '-0.04em' }}>Stampede</div>
			</div>
			<div style={{ fontFamily: display, fontWeight: 500, fontSize: 44, color: colors.muted, opacity: late, transform: `translateY(${(1 - late) * 16}px)` }}>{tagline}</div>
			{footer && <div style={{ marginTop: 26, fontFamily: text, fontSize: 28, color: colors.ember, opacity: late }}>{footer}</div>}
		</AbsoluteFill>
	);
};

const chatting = { keys: chatKeys(chat), events: on(chat, 'desktop') };
const typed = find(chat, 'desktop', (event) => event.type === 'type');
const sent = mark(chat, 'desktop', 'sent');
const scout = focus(chat, 'subagent');
const mockup = focus(chat, 'mockup');
const question = focus(chat, 'question');
const answered = mark(chat, 'desktop', 'answered');
const approval = focus(chat, 'approval');
const chatShots = [
	zoom(mark(chat, 'desktop', 'start'), 1),
	zoom(typed.t - 0.4, 1),
	zoom(typed.t + 0.5, 1.3, typed),
	zoom(sent + 0.3, 1.3, typed),
	zoom(sent + 1.4, 1),
	zoom(scout.t, 1),
	zoom(scout.t + 0.6, 1.15, scout),
	zoom(mark(chat, 'desktop', 'scouted'), 1.15, scout),
	zoom(mark(chat, 'desktop', 'scouted') + 0.8, 1),
	zoom(mockup.t, 1),
	zoom(mockup.t + 0.7, 1.3, mockup),
	zoom(question.t, 1.3, mockup),
	zoom(question.t + 0.6, 1.35, question),
	zoom(answered + 0.3, 1.35, question),
	zoom(answered + 1.2, 1),
	zoom(approval.t, 1),
	zoom(approval.t + 0.8, 1.5, approval)
];

export const Chatting = () => {
	const { t, at } = useClock(chatting.keys);
	return (
		<AbsoluteFill>
			<Screen scene="chat" frames={chatDesktop as Frames} events={chatting.events} t={t} shots={chatShots} />
			<Lower from={0} to={at(sent)}>Start a Chat in any project.</Lower>
			<Lower from={at(sent)} to={at(mockup.t)}>Claude explores with subagents.</Lower>
			<Lower from={at(mockup.t)} to={at(question.t)}>It sketches mockups right in the chat,</Lower>
			<Lower from={at(question.t)} to={at(answered + 0.6)}>and asks when a choice is yours.</Lower>
			<Lower from={at(answered + 0.6)} to={at(approval.t)}>Then it gets to work. Live.</Lower>
			<Lower from={at(approval.t)} to={seconds(length('chat'))}>Commands wait for your OK.</Lower>
		</AbsoluteFill>
	);
};

const phone = { width: 448, height: 998, keys: phoneKeys(chat), events: on(chat, 'phone') };
const screen = { width: 390, height: 390 * phone.height / phone.width };
const tapped = find(chat, 'phone', (event) => event.type === 'tap');

export const Handset = () => {
	const now = useCurrentFrame() / fps;
	const t = footageAt(phone.keys, now - lock.length);
	const tap = lock.length + outputAt(phone.keys, tapped.t);
	const reveal = interpolate(now, [lock.open + 0.1, lock.length], [0, 1], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' });
	const unlock = { x: screen.width / 2, y: 80, t: lock.open, type: 'tap', device: 'phone' };
	const card = centre(focus(chat, 'approval', 'phone').box!);
	const push = interpolate(now, [tap - 1.6, tap - 0.8, tap + 1.0, tap + 1.8], [1, 1.25, 1.25, 1], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: ease });
	const ratio = screen.width / phone.width;
	return (
		<AbsoluteFill>
			<div style={{ position: 'absolute', right: 250, top: (1080 - screen.height - 54) / 2, transform: `scale(${push})`, transformOrigin: `${12 + card.x * ratio}px ${42 + card.y * ratio}px` }}>
				<Phone width={screen.width} height={screen.height}>
					<div style={{ transform: `scale(${ratio})`, transformOrigin: '0 0', width: phone.width, height: phone.height, position: 'absolute' }}>
						<Footage scene="chat" device="phone" frames={chatPhone as Frames} t={t} />
						<Ripples events={phone.events} t={t} size={60} />
					</div>
					{reveal < 1 && (
						<div style={{ position: 'absolute', inset: 0, opacity: 1 - reveal, transform: `scale(${1 + reveal * 0.08})` }}>
							<LockScreen width={screen.width} height={screen.height} notify={lock.notify} open={lock.open} />
							<Ripples events={[unlock]} t={now} size={60} />
						</div>
					)}
				</Phone>
			</div>
			<Caption from={seconds(0.2)} to={seconds(tap - 0.6)} style={side}>
				Step away.<br /><span style={{ color: colors.muted, fontWeight: 500 }}>Your phone gets the ping.</span>
			</Caption>
			<Caption from={seconds(tap - 0.6)} to={seconds(length('phone'))} style={side}>Approve from<br />anywhere.</Caption>
		</AbsoluteFill>
	);
};

const side = { left: 200, top: 400, width: 820, fontSize: 92, fontWeight: 800, lineHeight: 1.02 } as const;

const reviewing = { keys: reviewKeys(chat), events: on(chat, 'desktop') };
const clicks = chat.filter((event) => event.type === 'click' && event.t > mark(chat, 'desktop', 'review'));
const [, , , draft, commit] = clicks.map((event) => event.t);
const message = focus(chat, 'message');
const reviewShots = [
	zoom(mark(chat, 'desktop', 'review'), 1),
	zoom(focus(chat, 'diff').t + 1.6, 1.12, focus(chat, 'diff')),
	zoom(draft - 0.3, 1),
	zoom(message.t, 1.6, message),
	zoom(commit + 0.4, 1.6, message),
	zoom(commit + 1.2, 1.3, focus(chat, 'history'))
];

export const Reviewing = () => {
	const { t, at } = useClock(reviewing.keys);
	return (
		<AbsoluteFill>
			<Screen scene="chat" frames={chatDesktop as Frames} events={reviewing.events} t={t} shots={reviewShots} rest={{ x: 1000, y: 330 }} />
			<Lower from={0} to={at(draft)}>Review every change side by side.</Lower>
			<Lower from={at(draft)} to={at(commit)}>A fast model drafts the commit message.</Lower>
			<Lower from={at(commit)} to={seconds(length('review'))}>Commit, and it is in your history.</Lower>
		</AbsoluteFill>
	);
};

const splitting = { keys: splitKeys(split), events: on(split, 'desktop') };
const splitShots = [zoom(mark(split, 'desktop', 'start'), 1)];

export const Splitting = () => {
	const { t } = useClock(splitting.keys);
	return (
		<AbsoluteFill>
			<Screen scene="split" frames={splitDesktop as Frames} events={splitting.events} t={t} shots={splitShots} rest={{ x: 1400, y: 900 }} />
			<Lower from={0} to={seconds(length('split'))}>Claude Code or Codex. Same interface.</Lower>
		</AbsoluteFill>
	);
};

const epics = { keys: epicKeys(epic), events: on(epic, 'desktop') };
const started = mark(epic, 'desktop', 'started');
const review = focus(epic, 'review');
const land = mark(epic, 'desktop', 'land');
const confirm = focus(epic, 'confirm');
const landed = focus(epic, 'landed');
const epicShots = [
	zoom(mark(epic, 'desktop', 'start'), 1.4, focus(epic, 'run')),
	zoom(started + 0.3, 1.4, focus(epic, 'run')),
	zoom(started + 1.2, 1),
	zoom(review.t, 1),
	zoom(review.t + 0.8, 1.3, review),
	zoom(land - 0.2, 1.3, review),
	zoom(land + 0.4, 1),
	zoom(confirm.t, 1),
	zoom(confirm.t + 0.4, 1.3, confirm),
	zoom(landed.t - 0.3, 1.3, confirm),
	zoom(landed.t + 0.3, 1.3, landed)
];

export const Running = () => {
	const { t, at } = useClock(epics.keys);
	return (
		<AbsoluteFill>
			<Screen scene="epic" frames={epicDesktop as Frames} events={epics.events} t={t} shots={epicShots} rest={{ x: 700, y: 300 }} />
			<Lower from={0} to={at(started)}>Plan bigger work as an Epic.</Lower>
			<Lower from={at(started)} to={at(review.t)}>A Run works the Tickets in parallel worktrees.</Lower>
			<Lower from={at(review.t)} to={at(land)}>Reviewers check it. A Remediator fixes it.</Lower>
			<Lower from={at(land)} to={seconds(length('epic'))}>Land it when it is ready.</Lower>
		</AbsoluteFill>
	);
};

const fleets = { keys: fleetKeys(fleet), events: on(fleet, 'desktop') };
const attention = focus(fleet, 'attention');
const asked = focus(fleet, 'question');
const drawer = mark(fleet, 'desktop', 'drawer');
const fleetShots = [
	zoom(mark(fleet, 'desktop', 'start'), 1),
	zoom(attention.t - 0.2, 1),
	zoom(attention.t + 0.3, 1.45, attention),
	zoom(asked.t - 0.1, 1.45, attention),
	zoom(asked.t + 0.6, 1.3, asked),
	zoom(drawer - 0.5, 1.3, asked),
	zoom(drawer + 0.1, 1)
];

export const Fleet = () => {
	const { t, at } = useClock(fleets.keys);
	return (
		<AbsoluteFill>
			<Screen scene="fleet" frames={fleetDesktop as Frames} events={fleets.events} t={t} shots={fleetShots} rest={{ x: 700, y: 420 }} />
			<Lower from={0} to={at(drawer)}>Every project and agent in one place.</Lower>
			<Lower from={at(drawer)} to={seconds(length('fleet'))}>And a terminal, one keystroke away.</Lower>
		</AbsoluteFill>
	);
};

export const Themes = () => {
	const now = useCurrentFrame() / fps;
	const stills = (themeStills as Frames & { frames: { name: string }[] }).frames;
	const still = stills[Math.min(stills.length - 1, Math.floor(now / (bar / 4)))];
	return (
		<AbsoluteFill>
			<Window>
				<Img src={staticFile(`footage/themes/desktop/${still.file}`)} style={{ position: 'absolute', inset: 0, width: '100%', height: '100%' }} />
			</Window>
			<Sequence key={still.file} from={seconds(stills.indexOf(still) * bar / 4)}>
				<Chip>{still.name}</Chip>
			</Sequence>
			<Lower from={0} to={seconds(length('themes'))}>460+ themes, light and dark.</Lower>
		</AbsoluteFill>
	);
};

export const Edit = ({ cut, parts, children }: { cut: Cut; parts: Partial<Record<Section, ReactNode>>; children?: ReactNode }) => (
	<AbsoluteFill style={{ background: colors.ground }}>
		<Backdrop />
		{cuts[cut].map((name) => <Part key={name} cut={cut} name={name}>{parts[name]}</Part>)}
		{children}
		<Audio src={staticFile(`music/${cut.toLowerCase()}.wav`)} />
	</AbsoluteFill>
);

export const Promo = () => (
	<Edit cut="Promo" parts={{
		intro: <Title tagline="A self-hosted IDE for directing AI coding agents." />,
		chat: <Chatting />,
		phone: <Handset />,
		review: <Reviewing />,
		split: <Splitting />,
		epic: <Running />,
		fleet: <Fleet />,
		themes: <Themes />,
		outro: <Title tagline="Your machine. Your agents. Any browser." footer="github.com/d-flood/stampede" />
	}} />
);
