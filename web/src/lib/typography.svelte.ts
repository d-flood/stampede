import { form } from '#lib/form.ts';

const key = 'stampede.text-size';
export const typography = $state<{ size: number | null }>({ size: null });

export function textSize(desktop = 13): number {
	return typography.size ?? (form() === 'phone' ? 16 : desktop);
}

export function loadTextSize() {
	try {
		const size = Number(localStorage.getItem(key));
		apply(Number.isInteger(size) && size >= 12 && size <= 22 ? size : null);
	} catch {
		apply(null);
	}
}

export function selectTextSize(size: number | null) {
	apply(size);
	try {
		if (size === null) localStorage.removeItem(key);
		else localStorage.setItem(key, String(size));
	} catch {}
}

function apply(size: number | null) {
	typography.size = size;
	const style = document.documentElement.style;
	if (size === null) style.removeProperty('--text-size');
	else style.setProperty('--text-size', `${size}px`);
}
