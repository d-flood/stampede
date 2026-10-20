import type { ITheme } from 'ghostty-web';
import { isDark } from '#lib/scheme.svelte.ts';
import { loadTextSize, textSize } from '#lib/typography.svelte.ts';

type Palette = {
	name: string | null;
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
export type Mode = { value: string; label: string; available: boolean };
export type Clip = { name: string; text: string };

let revision = $state(0);
let faces: FontFace[] = [];
let uiFaces: FontFace[] = [];
let configured = $state.raw<Rig[]>([]);
let offered = $state.raw<Template[]>([]);
let kept = $state.raw<Clip[]>([]);
let chord = $state('ctrl+;');
let extras = $state.raw<string[]>([]);
let scrollback = $state(10_000);
let named = $state<string | null>(null);
let committer = $state<string | null>(null);
let offeredModes = $state.raw<Partial<Record<Agent, Mode[]>>>({});
let savedTheme: ThemeResponse | undefined;
let previewing = false;
let savedUiFont: Font | null = null;
let previewingFont = false;

export const editor = $state<EditorSettings>({ keymap: 'standard' });
export const selectedTheme = $state<{ name: string | null; light: string | null; dark: string | null }>({ name: null, light: null, dark: null });
export const uiFont = $state<{ family: string | null }>({ family: null });

async function get<T>(path: string): Promise<T> {
	const response = await fetch(path);
	if (!response.ok) throw new Error(`${path}: ${response.status}`);
	return response.json();
}

export async function loadConfig() {
	loadTextSize();
	const [name, theme, font, ui, rigs, settings, leaderKey, keys, lines, listed, commitRigName, agentModes, saved] = await Promise.all([
		get<string | null>('/api/host-name'),
		get<ThemeResponse>('/api/theme'),
		get<Font>('/api/font'),
		get<Font | null>('/api/ui-font'),
		get<Rig[]>('/api/rigs'),
		get<EditorSettings>('/api/editor'),
		get<string>('/api/leader'),
		get<Keybar>('/api/keybar'),
		get<number>('/api/scrollback'),
		get<Template[]>('/api/templates'),
		get<string | null>('/api/commit-rig'),
		get<Record<Agent, Mode[]>>('/api/modes'),
		get<Clip[]>('/api/clips')
	]);
	named = name;
	committer = commitRigName;
	offeredModes = agentModes;
	editor.keymap = settings.keymap;
	chord = leaderKey;
	extras = keys.extra;
	scrollback = lines;
	keepTheme(theme);
	faces = register(faces, font);
	document.documentElement.style.setProperty('--font-mono', `"${font.family}", "Symbols Nerd Font Mono", monospace`);
	keepUiFont(ui);
	configured = rigs;
	offered = listed;
	kept = saved;
	revision++;
}

function register(previous: FontFace[], font: Font | null) {
	for (const face of previous) document.fonts.delete(face);
	const added = font ? font.faces.map(({ url, weight, style }) => new FontFace(font.family, `url("${url}")`, { weight, style })) : [];
	for (const face of added) document.fonts.add(face);
	return added;
}

function applyUiFont(font: Font | null) {
	uiFaces = register(uiFaces, font);
	const style = document.documentElement.style;
	if (font) style.setProperty('--font-ui', `"${font.family}", "Symbols Nerd Font Mono", sans-serif`);
	else style.removeProperty('--font-ui');
}

function keepUiFont(font: Font | null) {
	savedUiFont = font;
	uiFont.family = font?.family ?? null;
	if (!previewingFont) applyUiFont(font);
}

export async function selectUiFont(family: string | null) {
	const response = await fetch('/api/ui-font', {
		method: 'PUT',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(family)
	});
	if (!response.ok) throw new Error(await response.text());
	keepUiFont(await response.json());
}

export function beginFontPreview() {
	previewingFont = true;
	return preview(
		(family: string | null) => (family === null ? Promise.resolve(null) : get<Font>(`/api/fonts/${encodeURIComponent(family)}`)),
		applyUiFont,
		() => {
			previewingFont = false;
			applyUiFont(savedUiFont);
		}
	);
}

function keepTheme(theme: ThemeResponse) {
	savedTheme = theme;
	selectedTheme.name = theme.name;
	selectedTheme.light = theme.light.name;
	selectedTheme.dark = theme.dark.name;
	if (!previewing) applyTheme(theme);
}

export function themeLabel(): string {
	const { light, dark } = selectedTheme;
	return light === dark ? (light ?? 'Default') : `Light: ${light} · Dark: ${dark}`;
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
	keepTheme(await response.json());
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
	return preview(
		(name: string) => get<ThemeResponse>(`/api/themes/${encodeURIComponent(name)}`),
		applyTheme,
		() => {
			previewing = false;
			if (savedTheme) applyTheme(savedTheme);
		}
	);
}

function preview<Name, Value>(load: (name: Name) => Promise<Value>, apply: (value: Value) => void, restore: () => void) {
	let active = true;
	let request = 0;
	return {
		async preview(name: Name) {
			const current = ++request;
			try {
				const value = await load(name);
				if (!active || current !== request) return false;
				apply(value);
				return true;
			} catch (error) {
				if (!active || current !== request) return false;
				throw error;
			}
		},
		end() {
			active = false;
			restore();
		}
	};
}

export function commitRig(): string | null {
	return committer;
}

export async function saveCommitRig(name: string) {
	const response = await fetch('/api/commit-rig', {
		method: 'PUT',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(name)
	});
	if (!response.ok) throw new Error(await response.text());
	committer = await response.json();
}

export function modes(agent: Agent): Mode[] {
	return offeredModes[agent] ?? [];
}

export async function saveModes(agent: Agent, values: string[]) {
	const response = await fetch('/api/modes', {
		method: 'PUT',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ agent, modes: values })
	});
	if (!response.ok) throw new Error(await response.text());
	offeredModes = await response.json();
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

export async function reorderRigs(order: string[]) {
	const response = await fetch('/api/rigs/order', {
		method: 'PUT',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(order)
	});
	if (!response.ok) throw new Error(await response.text());
	configured = await response.json();
}

export function templates(): Template[] {
	return offered;
}

export function clips(): Clip[] {
	return kept;
}

export async function saveClip(clip: { original?: string; name: string; text: string }) {
	const response = await fetch('/api/clips', {
		method: 'PUT',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(clip)
	});
	if (!response.ok) throw new Error(await response.text());
	kept = await response.json();
}

export async function deleteClip(name: string) {
	const response = await fetch('/api/clips', {
		method: 'DELETE',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(name)
	});
	if (!response.ok) throw new Error(await response.text());
	kept = await response.json();
}

export async function reorderClips(order: string[]) {
	const response = await fetch('/api/clips/order', {
		method: 'PUT',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(order)
	});
	if (!response.ok) throw new Error(await response.text());
	kept = await response.json();
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
	void isDark();
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
