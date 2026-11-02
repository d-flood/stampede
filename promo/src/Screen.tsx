import type { ReactNode } from 'react';
import { interpolate, spring, useCurrentFrame, useVideoConfig } from 'remotion';
import { find, footageAt, fps, outputAt, type Event, type Key } from './edit.ts';
import { Cursor, Footage, Ripples, camera, centre, type Frames, type Shot } from './footage.tsx';
import { Browser, Caption, colors, text } from './parts.tsx';

export const frame = { left: 240, top: 28, width: 1440, height: 900 };
export const url = 'https://studio.tailnet.ts.net:8443';

export const seconds = (value: number) => Math.round(value * fps);
export const on = (timeline: Event[], device: string) => timeline.filter((event) => event.device === device);
export const focus = (timeline: Event[], name: string, device = 'desktop') => find(timeline, device, (event) => event.type === 'focus' && event.name === name);
export const zoom = (t: number, z: number, event?: Event): Shot => ({ t, z, ...(event ? centre(event.box!) : { x: 640, y: 400 }) });

export function useClock(keys: Key[]) {
	const now = useCurrentFrame();
	return { t: footageAt(keys, now / fps), at: (foot: number) => Math.round(outputAt(keys, foot) * fps) };
}

export const Window = ({ children }: { children: ReactNode }) => (
	<div style={{ position: 'absolute', left: frame.left, top: frame.top }}>
		<Browser width={frame.width} height={frame.height} url={url}>
			<div style={{ transform: `scale(${frame.width / 1280})`, transformOrigin: '0 0', width: 1280, height: 800, position: 'relative' }}>{children}</div>
		</Browser>
	</div>
);

export const Screen = ({ scene, frames, events, t, shots, rest = { x: 640, y: 400 } }: { scene: string; frames: Frames; events: Event[]; t: number; shots: Shot[]; rest?: { x: number; y: number } }) => (
	<Window>
		<div style={{ position: 'absolute', inset: 0, transformOrigin: '0 0', transform: camera(shots, t, 1280, 800) }}>
			<Footage scene={scene} device="desktop" frames={frames} t={t} />
			<Ripples events={events} t={t} color="236, 111, 45" />
			<Cursor events={events} t={t} rest={rest} />
		</div>
		<Keys events={events} t={t} />
	</Window>
);

const Keys = ({ events, t }: { events: Event[]; t: number }) => {
	const event = events.find((event) => event.type === 'keys' && t >= event.t && t < event.t + 1.6);
	if (!event) return null;
	const p = (t - event.t) / 1.6;
	const opacity = interpolate(p, [0, 0.1, 0.8, 1], [0, 1, 1, 0]);
	return (
		<div style={{ position: 'absolute', right: 32, bottom: 32, display: 'flex', gap: 10, opacity, transform: `translateY(${(1 - Math.min(1, p * 8)) * 12}px)` }}>
			{event.name!.split(' + ').map((key) => (
				<div key={key} style={{ minWidth: 56, padding: '10px 16px', borderRadius: 12, background: 'rgba(20, 18, 16, 0.92)', border: '1px solid rgba(255,255,255,0.18)', boxShadow: '0 6px 0 rgba(0,0,0,0.5)', color: colors.ink, fontFamily: text, fontWeight: 600, fontSize: 28, textAlign: 'center' }}>{key}</div>
			))}
		</div>
	);
};

export const Lower = ({ from, to, children }: { from: number; to: number; children: ReactNode }) => (
	<Caption from={from} to={to} style={{ left: 0, right: 0, bottom: 40, textAlign: 'center', fontSize: 48, fontWeight: 700 }}>{children}</Caption>
);

export const Chip = ({ children }: { children: ReactNode }) => {
	const now = useCurrentFrame();
	const { fps } = useVideoConfig();
	const enter = spring({ frame: now, fps, config: { damping: 18 } });
	return (
		<div style={{ position: 'absolute', right: frame.left + 24, top: frame.top + 64, padding: '10px 18px', borderRadius: 999, background: 'rgba(20, 18, 16, 0.9)', border: '1px solid rgba(255,255,255,0.16)', color: colors.ink, fontFamily: text, fontWeight: 600, fontSize: 24, opacity: enter }}>{children}</div>
	);
};
