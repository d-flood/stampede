import type { ITheme } from 'ghostty-web';
import { MediaQuery } from 'svelte/reactivity';

type Palette = {
	background: string;
	foreground: string;
	palette: string[];
	cursor: string;
	cursorText: string;
	selectionBackground: string;
	selectionForeground: string;
};
type ThemeResponse = { light: Palette; dark: Palette };
type Font = { family: string; faces: { url: string; weight: string; style: string }[] };

export type Appearance = { theme: ITheme; fontFamily: string };
export type Agent = 'claude-code' | 'codex';
export type Rig = { name: string; agent: Agent };

const dark = new MediaQuery('(prefers-color-scheme: dark)');
let revision = $state(0);
let faces: FontFace[] = [];
let configured = $state.raw<Rig[]>([]);

async function get<T>(path: string): Promise<T> {
	const response = await fetch(path);
	if (!response.ok) throw new Error(`${path}: ${response.status}`);
	return response.json();
}

export async function loadConfig() {
	const [theme, font, rigs] = await Promise.all([
		get<ThemeResponse>('/api/theme'),
		get<Font>('/api/font'),
		get<Rig[]>('/api/rigs')
	]);
	const style = document.documentElement.style;
	for (const scheme of ['light', 'dark'] as const) {
		const palette = theme[scheme];
		style.setProperty(`--${scheme}-bg`, palette.background);
		style.setProperty(`--${scheme}-fg`, palette.foreground);
		palette.palette.forEach((color, n) => style.setProperty(`--${scheme}-ansi-${n}`, color));
		style.setProperty(`--${scheme}-cursor`, palette.cursor);
		style.setProperty(`--${scheme}-selection-bg`, palette.selectionBackground);
		style.setProperty(`--${scheme}-selection-fg`, palette.selectionForeground);
	}
	for (const face of faces) document.fonts.delete(face);
	faces = font.faces.map(({ url, weight, style }) => new FontFace(font.family, `url("${url}")`, { weight, style }));
	for (const face of faces) document.fonts.add(face);
	style.setProperty('--font-mono', `"${font.family}", "Symbols Nerd Font Mono", monospace`);
	configured = rigs;
	revision++;
}

export function rigs(): Rig[] {
	return configured;
}

export function appearance(): Appearance {
	// Reading these re-runs the caller when the config or the color scheme changes.
	void revision;
	void dark.current;
	const style = getComputedStyle(document.documentElement);
	const token = (name: string) => style.getPropertyValue(name).trim();
	const ansi = (n: number) => token(`--ansi-${n}`);
	return {
		theme: {
			background: token('--color-bg'),
			foreground: token('--color-fg'),
			cursor: token('--color-cursor'),
			selectionBackground: token('--color-selection-bg'),
			selectionForeground: token('--color-selection-fg'),
			black: ansi(0),
			red: ansi(1),
			green: ansi(2),
			yellow: ansi(3),
			blue: ansi(4),
			magenta: ansi(5),
			cyan: ansi(6),
			white: ansi(7),
			brightBlack: ansi(8),
			brightRed: ansi(9),
			brightGreen: ansi(10),
			brightYellow: ansi(11),
			brightBlue: ansi(12),
			brightMagenta: ansi(13),
			brightCyan: ansi(14),
			brightWhite: ansi(15)
		},
		fontFamily: token('--font-mono')
	};
}
