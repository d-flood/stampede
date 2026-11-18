import { on } from 'svelte/events';

type Swipe = {
	/** Share of the element's width a slow swipe must cross to count. */
	share: number;
	/** Only swipes toward this side count; -1 is left. */
	direction?: -1 | 1;
	allow?(event: TouchEvent): boolean;
	drag(dx: number): void;
	release(dx: number, committed: boolean): void;
};

/** Plays keyframes into an element's resting state, unless reduced motion is on. */
export function settleInto(element: Element | null | undefined, keyframes: Keyframe[]) {
	if (!matchMedia('(prefers-reduced-motion: reduce)').matches) element?.animate(keyframes, { duration: 160, easing: 'cubic-bezier(0.2, 0.8, 0.3, 1)' });
}

/** Tracks one-finger sideways swipes, leaving vertical scrolling alone. */
export function swipe(options: Swipe) {
	return (element: HTMLElement) => {
		let gesture: { x: number; y: number; time: number; horizontal: boolean } | undefined;
		const start = (event: TouchEvent) => {
			const [touch] = event.touches;
			gesture = event.touches.length === 1 && (options.allow?.(event) ?? true) ? { x: touch.clientX, y: touch.clientY, time: performance.now(), horizontal: false } : undefined;
		};
		const move = (event: TouchEvent) => {
			if (!gesture) return;
			const dx = event.touches[0].clientX - gesture.x;
			const dy = event.touches[0].clientY - gesture.y;
			if (!gesture.horizontal) {
				if (Math.max(Math.abs(dx), Math.abs(dy)) < 8) return;
				if (Math.abs(dy) >= Math.abs(dx) || (options.direction && Math.sign(dx) !== options.direction) || !event.cancelable) {
					gesture = undefined;
					return;
				}
				gesture.horizontal = true;
			}
			event.preventDefault();
			options.drag(dx);
		};
		const release = (event: TouchEvent) => {
			if (gesture?.horizontal) {
				const dx = event.changedTouches[0].clientX - gesture.x;
				const distance = Math.abs(dx);
				const toward = !options.direction || Math.sign(dx) === options.direction;
				const far = distance > element.clientWidth * options.share || distance / (performance.now() - gesture.time) > 0.5;
				options.release(dx, event.type === 'touchend' && toward && far);
			}
			gesture = undefined;
		};
		const off = [
			on(element, 'touchstart', start, { passive: true }),
			on(element, 'touchmove', move, { passive: false }),
			on(element, 'touchend', release),
			on(element, 'touchcancel', release)
		];
		return () => off.forEach((listener) => listener());
	};
}
