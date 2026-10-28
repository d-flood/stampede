import type { FontRole } from '#lib/config.svelte.ts';
import { form } from '#lib/form.ts';

const keys: Record<FontRole, string> = { ui: 'stampede.text-size', chat: 'stampede.chat-size', code: 'stampede.code-size' };
const variables: Record<FontRole, string> = { ui: '--ui-size', chat: '--chat-size', code: '--code-size' };
export const defaultSizes: Record<FontRole, { desktop: number; phone: number }> = {
	ui: { desktop: 16, phone: 16 },
	chat: { desktop: 16, phone: 18 },
	code: { desktop: 16, phone: 16 }
};
export const typography = $state<Record<FontRole, number | null>>({ ui: null, chat: null, code: null });

export function textSize(role: FontRole): number {
	return typography[role] ?? defaultSizes[role][form()];
}

export function loadTextSizes() {
	for (const role of Object.keys(keys) as FontRole[]) {
		try {
			const size = Number(localStorage.getItem(keys[role]));
			apply(role, Number.isInteger(size) && size >= 10 && size <= 28 ? size : null);
		} catch {
			apply(role, null);
		}
	}
}

export function selectTextSize(role: FontRole, size: number | null) {
	apply(role, size);
	try {
		if (size === null) localStorage.removeItem(keys[role]);
		else localStorage.setItem(keys[role], String(size));
	} catch {}
}

function apply(role: FontRole, size: number | null) {
	typography[role] = size;
	const style = document.documentElement.style;
	if (size === null) style.removeProperty(variables[role]);
	else style.setProperty(variables[role], `${size}px`);
}
