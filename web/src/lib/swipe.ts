import { on } from 'svelte/events';

type Swipe = {
	/** Share of the element's size along the swipe that a slow swipe must cross to count. */
	share: number;
	/** Tracks up and down instead of sideways. */
	vertical?: boolean;
	/** Only swipes toward this end count; -1 is left, or up when vertical. */
	direction?: -1 | 1;
	allow?(event: TouchEvent): boolean;
	/** Checked on every move; false lets the browser have the rest of the touch. */
	holds?(event: TouchEvent, distance: number): boolean;
	drag(distance: number): void;
	release(distance: number, committed: boolean): void;
};

/** Plays keyframes into an element's resting state, unless reduced motion is on. */
export function settleInto(element: Element | null | undefined, keyframes: Keyframe[]) {
	if (!matchMedia('(prefers-reduced-motion: reduce)').matches) element?.animate(keyframes, { duration: 160, easing: 'cubic-bezier(0.2, 0.8, 0.3, 1)' });
}

/** Tracks one-finger swipes along one axis, leaving scrolling along the other alone. */
export function swipe(options: Swipe) {
	return (element: HTMLElement) => {
		let gesture: { x: number; y: number; time: number; engaged: boolean } | undefined;
		const along = (touch: Touch) => (options.vertical ? touch.clientY - gesture!.y : touch.clientX - gesture!.x);
		const start = (event: TouchEvent) => {
			const [touch] = event.touches;
			gesture = event.touches.length === 1 && (options.allow?.(event) ?? true) ? { x: touch.clientX, y: touch.clientY, time: performance.now(), engaged: false } : undefined;
		};
		const move = (event: TouchEvent) => {
			if (!gesture) return;
			const [touch] = event.touches;
			const distance = along(touch);
			if (!gesture.engaged) {
				const across = options.vertical ? touch.clientX - gesture.x : touch.clientY - gesture.y;
				if (Math.max(Math.abs(distance), Math.abs(across)) < 8) return;
				if (Math.abs(across) >= Math.abs(distance) || (options.direction && Math.sign(distance) !== options.direction) || !event.cancelable) {
					gesture = undefined;
					return;
				}
				gesture.engaged = true;
			}
			if (options.holds?.(event, distance) === false) {
				options.release(distance, false);
				gesture = undefined;
				return;
			}
			event.preventDefault();
			options.drag(distance);
		};
		const release = (event: TouchEvent) => {
			if (gesture?.engaged) {
				const distance = along(event.changedTouches[0]);
				const toward = !options.direction || Math.sign(distance) === options.direction;
				const size = options.vertical ? element.clientHeight : element.clientWidth;
				const far = Math.abs(distance) > size * options.share || Math.abs(distance) / (performance.now() - gesture.time) > 0.5;
				options.release(distance, event.type === 'touchend' && toward && far);
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

/** Whether a touch began in something that scrolls sideways, which keeps the swipe for itself. */
export function scrolls(event: TouchEvent, distance = 0) {
	const within = event.currentTarget as Element;
	for (let element = event.target as Element | null; element && element !== within; element = element.parentElement) {
		const overflow = element.scrollWidth - element.clientWidth;
		if (overflow <= 0 || !/auto|scroll/.test(getComputedStyle(element).overflowX)) continue;
		if (!distance || (distance > 0 ? element.scrollLeft >= 1 : element.scrollLeft <= overflow - 1)) return true;
	}
	return false;
}
