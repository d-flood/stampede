import type { ITheme } from 'ghostty-web';
import { MediaQuery } from 'svelte/reactivity';
import { loadTextSize, textSize } from '#lib/typography.svelte.ts';

type Palette = {
	background: string;
	foreground: string;
	palette: string[];
	cursor: string;
	cursorText: string;
	selectionBackground: string;
	selectionForeground: string;
};
type ThemeResponse = { name: string | null; light: Palette; dark: Palette };
type Font = { family: string; faces: { url: string; weight: string; style: string }[] };
type EditorSettings = { keymap: 'standard' | 'vim' };
type Keybar = { extra: string[] };

export type Appearance = { theme: ITheme; fontFamily: string; fontSize: number; scrollback: number };
export type Agent = 'claude-code' | 'codex' | 'copilot';
export const agentNames: Record<Agent, string> = { 'claude-code': 'Claude Code', codex: 'Codex', copilot: 'GitHub Copilot' };
export type Rig = { name: string; agent: Agent; shortcut?: string; model?: string; effort?: string; mode?: string };
export type Template = { name: string; description: string };

const dark = new MediaQuery('(prefers-color-scheme: dark)');
let revision = $state(0);
let faces: FontFace[] = [];
let configured = $state.raw<Rig[]>([]);
let offered = $state.raw<Template[]>([]);
let chord = $state('ctrl+;');
let extras = $state.raw<string[]>([]);
let scrollback = $state(10_000);
let named = $state<string | null>(null);
let savedTheme: ThemeResponse | undefined;
let previewing = false;

export const editor = $state<EditorSettings>({ keymap: 'standard' });
export const selectedTheme = $state<{ name: string | null }>({ name: null });

async function get<T>(path: string): Promise<T> {
	const response = await fetch(path);
	if (!response.ok) throw new Error(`${path}: ${response.status}`);
	return response.json();
}

export async function loadConfig() {
	loadTextSize();
	const [name, theme, font, rigs, settings, leaderKey, keys, lines, listed] = await Promise.all([
		get<string | null>('/api/host-name'),
		get<ThemeResponse>('/api/theme'),
		get<Font>('/api/font'),
		get<Rig[]>('/api/rigs'),
		get<EditorSettings>('/api/editor'),
		get<string>('/api/leader'),
		get<Keybar>('/api/keybar'),
		get<number>('/api/scrollback'),
		get<Template[]>('/api/templates')
	]);
	named = name;
	editor.keymap = settings.keymap;
	chord = leaderKey;
	extras = keys.extra;
	scrollback = lines;
	savedTheme = theme;
	selectedTheme.name = theme.name;
	if (!previewing) applyTheme(theme);
	const style = document.documentElement.style;
	for (const face of faces) document.fonts.delete(face);
	faces = font.faces.map(({ url, weight, style }) => new FontFace(font.family, `url("${url}")`, { weight, style }));
	for (const face of faces) document.fonts.add(face);
	style.setProperty('--font-mono', `"${font.family}", "Symbols Nerd Font Mono", monospace`);
	configured = rigs;
	offered = listed;
	revision++;
}

function applyTheme(theme: ThemeResponse) {
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
	revision++;
}

export async function selectTheme(name: string) {
	const response = await fetch('/api/theme', {
		method: 'PUT',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(name)
	});
	if (!response.ok) throw new Error(await response.text());
	const theme: ThemeResponse = await response.json();
	savedTheme = theme;
	selectedTheme.name = theme.name;
	if (!previewing) applyTheme(theme);
}

export function hostName(): string | null {
	return named;
}

export async function saveHostName(name: string | null) {
	const response = await fetch('/api/host-name', {
		method: 'PUT',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(name)
	});
	if (!response.ok) throw new Error(await response.text());
	named = await response.json();
}

export function beginThemePreview() {
	previewing = true;
	let active = true;
	let request = 0;
	return {
		async preview(name: string) {
			const current = ++request;
			try {
				const theme = await get<ThemeResponse>(`/api/themes/${encodeURIComponent(name)}`);
				if (!active || current !== request) return false;
				applyTheme(theme);
				return true;
			} catch (error) {
				if (!active || current !== request) return false;
				throw error;
			}
		},
		end() {
			active = false;
			previewing = false;
			if (savedTheme) applyTheme(savedTheme);
		}
	};
}

export function rigs(): Rig[] {
	return configured;
}

export async function saveRig(settings: { original?: string; name: string; agent: Agent; model?: string; effort?: string; mode: string }) {
	const response = await fetch('/api/rigs', {
		method: 'PUT',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(settings)
	});
	if (!response.ok) throw new Error(await response.text());
	configured = await response.json();
}

export async function deleteRig(name: string) {
	const response = await fetch('/api/rigs', {
		method: 'DELETE',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(name)
	});
	if (!response.ok) throw new Error(await response.text());
	configured = await response.json();
}

export function templates(): Template[] {
	return offered;
}

export function leader(): string {
	return chord;
}

export function keybarExtras(): string[] {
	return extras;
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
		fontFamily: token('--font-mono'),
		fontSize: textSize(15),
		scrollback
	};
}
