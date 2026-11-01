import { Img, interpolate, staticFile } from 'remotion';
import type { Event } from './edit.ts';

export type Frames = { viewport: { width: number; height: number }; scale: number; frames: { t: number; file: string }[] };
export type Shot = { t: number; z: number; x: number; y: number };
type Point = { x: number; y: number };

export const ease = (p: number) => (p < 0.5 ? 4 * p ** 3 : 1 - (-2 * p + 2) ** 3 / 2);
const clamp = (value: number) => Math.min(1, Math.max(0, value));

export function centre(box: NonNullable<Event['box']>): Point {
	return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
}

export const Footage = ({ scene, device, frames, t }: { scene: string; device: string; frames: Frames; t: number }) => {
	let low = 0;
	let high = frames.frames.length - 1;
	while (low < high) {
		const middle = Math.ceil((low + high) / 2);
		if (frames.frames[middle].t <= t) low = middle;
		else high = middle - 1;
	}
	return <Img src={staticFile(`footage/${scene}/${device}/${frames.frames[low].file}`)} style={{ position: 'absolute', inset: 0, width: '100%', height: '100%' }} />;
};

export function camera(shots: Shot[], t: number, width: number, height: number) {
	let index = 0;
	while (index < shots.length - 2 && shots[index + 1].t <= t) index++;
	const [a, b] = [shots[index], shots[index + 1] ?? shots[index]];
	const p = ease(clamp((t - a.t) / Math.max(0.001, b.t - a.t)));
	const z = a.z + (b.z - a.z) * p;
	const x = a.x + (b.x - a.x) * p;
	const y = a.y + (b.y - a.y) * p;
	const left = Math.min(0, Math.max(width - width * z, width / 2 - x * z));
	const top = Math.min(0, Math.max(height - height * z, height / 2 - y * z));
	return `translate(${left}px, ${top}px) scale(${z})`;
}

export function pointer(events: Event[], t: number, rest: Point) {
	let current = rest;
	let pressed = 0;
	for (const event of events) {
		if (event.type === 'click' && t >= event.t) pressed = Math.max(pressed, 1 - clamp((t - event.t) / 0.25));
	}
	for (const [index, event] of events.entries()) {
		if (event.type !== 'move' || t < event.t) continue;
		const target = { x: event.x!, y: event.y! };
		const next = events.slice(index + 1).find((later) => later.type !== 'mark' && later.type !== 'focus');
		const arrival = Math.min(event.t + 0.8, next && ['click', 'tap'].includes(next.type) ? next.t : Infinity);
		if (t >= arrival) {
			current = target;
			continue;
		}
		const p = ease(clamp((t - event.t) / (arrival - event.t)));
		return { x: current.x + (target.x - current.x) * p, y: current.y + (target.y - current.y) * p, pressed };
	}
	return { ...current, pressed };
}

export const Cursor = ({ events, t, rest }: { events: Event[]; t: number; rest: Point }) => {
	const { x, y, pressed } = pointer(events, t, rest);
	const moved = events.filter((event) => ['move', 'click'].includes(event.type) && event.t <= t).at(-1)?.t ?? -Infinity;
	const idle = clamp((t - moved - 1.8) / 0.4);
	return (
		<svg width={26} height={34} viewBox="-1 -1 15 21" style={{ position: 'absolute', left: x - 2, top: y - 2, opacity: 1 - idle, transform: `scale(${1 - pressed * 0.15})`, transformOrigin: '2px 2px', filter: 'drop-shadow(0 2px 3px rgba(0,0,0,0.45))' }}>
			<path d="M0 0 L0 16 L4.2 12.3 L7 18.6 L9.8 17.4 L7.1 11.2 L12.6 11.2 Z" fill="#fff" stroke="#111" strokeWidth={1.1} strokeLinejoin="round" />
		</svg>
	);
};

export const Ripples = ({ events, t, size = 46, color = '255, 255, 255' }: { events: Event[]; t: number; size?: number; color?: string }) => (
	<>
		{events.filter((event) => ['click', 'tap'].includes(event.type) && t >= event.t && t < event.t + 0.6).map((event) => {
			const at = event.type === 'tap' ? event : pointer(events, event.t, { x: 0, y: 0 });
			const p = (t - event.t) / 0.6;
			const radius = interpolate(p, [0, 1], [size * 0.25, size], { easing: (v) => 1 - (1 - v) ** 3 });
			return (
				<div key={event.t} style={{ position: 'absolute', left: at.x! - radius, top: at.y! - radius, width: radius * 2, height: radius * 2, borderRadius: '50%', border: `3px solid rgba(${color}, ${0.8 * (1 - p)})`, background: `rgba(${color}, ${0.25 * (1 - p)})` }} />
			);
		})}
	</>
);
