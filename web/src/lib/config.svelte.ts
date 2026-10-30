import type { ITheme } from 'ghostty-web';
import { isDark } from '#lib/scheme.svelte.ts';
import { loadSidebarSide } from '#lib/sidebar.svelte.ts';
import { loadTextSizes, textSize } from '#lib/typography.svelte.ts';

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
type Font = { family: string; faces: { url: string; weight: string; style: string; stretch: string }[] };
type EditorSettings = { keymap: 'standard' | 'vim' };
type Keybar = { extra: string[] };

export type Appearance = { theme: ITheme; fontFamily: string; fontSize: number; scrollback: number };
export type FontRole = 'ui' | 'chat' | 'code';
export type Agent = 'claude-code' | 'codex' | 'copilot';
export const agentNames: Record<Agent, string> = { 'claude-code': 'Claude Code', codex: 'Codex', copilot: 'GitHub Copilot' };
export type Rig = { name: string; agent: Agent; shortcut?: string; model?: string; effort?: string; mode?: string };
export type Template = { name: string; description: string };
export type Mode = { value: string; label: string; available: boolean };
export type Clip = { name: string; text: string };

let revision = $state(0);
let faces: FontFace[] = [];
const roleFaces: Record<FontRole, FontFace[]> = { ui: [], chat: [], code: [] };
export const fontVariables: Record<FontRole, string> = { ui: '--font-ui', chat: '--font-chat', code: '--font-mono' };
export const fontNames: Record<FontRole, string> = { ui: 'UI font', chat: 'Chat font', code: 'Code font' };
const generics: Record<FontRole, string> = { ui: 'sans-serif', chat: 'serif', code: 'monospace' };
let configured = $state.raw<Rig[]>([]);
let offered = $state.raw<Template[]>([]);
let kept = $state.raw<Clip[]>([]);
let chord = $state('ctrl+;');
let extras = $state.raw<string[]>([]);
let scrollback = $state(10_000);
let named = $state<string | null>(null);
let fetching = $state(true);
let committer = $state<string | null>(null);
let instructions = $state('');
let offeredModes = $state.raw<Partial<Record<Agent, Mode[]>>>({});
let savedTheme: ThemeResponse | undefined;
let previewing = false;
const savedFonts: Record<FontRole, Font | null> = { ui: null, chat: null, code: null };
let previewingFont: FontRole | null = null;

export const editor = $state<EditorSettings>({ keymap: 'standard' });
export const selectedTheme = $state<{ name: string | null; light: string | null; dark: string | null }>({ name: null, light: null, dark: null });
export const fonts = $state<Record<FontRole, string | null>>({ ui: null, chat: null, code: null });
export const defaultFonts = $state<Record<FontRole, string>>({ ui: 'Fira Sans', chat: 'Literata', code: 'JetBrains Mono' });

async function get<T>(path: string): Promise<T> {
	const response = await fetch(path);
	if (!response.ok) throw new Error(`${path}: ${response.status}`);
	return response.json();
}

export async function loadConfig() {
	loadTextSizes();
	loadSidebarSide();
	const [name, theme, font, ui, chat, code, rigs, settings, leaderKey, keys, lines, listed, commitRigName, commitText, agentModes, saved, autoFetching] = await Promise.all([
		get<string | null>('/api/host-name'),
		get<ThemeResponse>('/api/theme'),
		get<Font>('/api/font'),
		get<Font | null>('/api/font-choice/ui'),
		get<Font | null>('/api/font-choice/chat'),
		get<Font | null>('/api/font-choice/code'),
		get<Rig[]>('/api/rigs'),
		get<EditorSettings>('/api/editor'),
		get<string>('/api/leader'),
		get<Keybar>('/api/keybar'),
		get<number>('/api/scrollback'),
		get<Template[]>('/api/templates'),
		get<string | null>('/api/commit-rig'),
		get<string>('/api/commit-instructions'),
		get<Record<Agent, Mode[]>>('/api/modes'),
		get<Clip[]>('/api/clips'),
		get<boolean>('/api/auto-fetch')
	]);
	named = name;
	fetching = autoFetching;
	committer = commitRigName;
	instructions = commitText;
	offeredModes = agentModes;
	editor.keymap = settings.keymap;
	chord = leaderKey;
	extras = keys.extra;
	scrollback = lines;
	keepTheme(theme);
	faces = register(faces, font);
	defaultFonts.code = font.family;
	keepFont('ui', ui);
	keepFont('chat', chat);
	keepFont('code', code);
	configured = rigs;
	offered = listed;
	kept = saved;
	revision++;
}

function register(previous: FontFace[], font: Font | null) {
	for (const face of previous) document.fonts.delete(face);
	const added = font ? font.faces.map(({ url, weight, style, stretch }) => new FontFace(font.family, `url("${url}")`, { weight, style, stretch })) : [];
	for (const face of added) document.fonts.add(face);
	return added;
}

function applyFont(role: FontRole, font: Font | null) {
	roleFaces[role] = register(roleFaces[role], font);
	const family = font?.family ?? (role === 'code' ? defaultFonts.code : null);
	const style = document.documentElement.style;
	if (family) style.setProperty(fontVariables[role], `"${family}", "Symbols Nerd Font Mono", ${generics[role]}`);
	else style.removeProperty(fontVariables[role]);
	revision++;
}

function keepFont(role: FontRole, font: Font | null) {
	savedFonts[role] = font;
	fonts[role] = font?.family ?? null;
	if (previewingFont !== role) applyFont(role, font);
}

export async function selectFont(role: FontRole, family: string | null) {
	const response = await fetch(`/api/font-choice/${role}`, {
		method: 'PUT',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(family)
	});
	if (!response.ok) throw new Error(await response.text());
	keepFont(role, await response.json());
}

export function beginFontPreview(role: FontRole) {
	previewingFont = role;
	return preview(
		(family: string | null) => (family === null ? Promise.resolve(null) : get<Font>(`/api/fonts/${encodeURIComponent(family)}`)),
		(font: Font | null) => applyFont(role, font),
		() => {
			previewingFont = null;
			applyFont(role, savedFonts[role]);
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

function luminance(hex: string): number {
	const [r, g, b] = [1, 3, 5].map((i) => {
		const c = parseInt(hex.slice(i, i + 2), 16) / 255;
		return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
	});
	return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function contrast(a: string, b: string): number {
	const [light, dark] = [luminance(a), luminance(b)].sort((x, y) => y - x);
	return (light + 0.05) / (dark + 0.05);
}

// Inverting themes rely on selection-foreground, which the editor can't apply over syntax colors, so tint instead.
function editorSelection({ foreground, background, selectionBackground }: Palette): string {
	return contrast(foreground, selectionBackground) < 3
		? `color-mix(in oklab, ${selectionBackground} 30%, ${background})`
		: selectionBackground;
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
		style.setProperty(`--${scheme}-editor-selection`, editorSelection(palette));
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

export function autoFetch(): boolean {
	return fetching;
}

export async function saveAutoFetch(on: boolean) {
	const response = await fetch('/api/auto-fetch', {
		method: 'PUT',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(on)
	});
	if (!response.ok) throw new Error(await response.text());
	fetching = await response.json();
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

export function commitInstructions(): string {
	return instructions;
}

export async function saveCommitInstructions(text: string) {
	const response = await fetch('/api/commit-instructions', {
		method: 'PUT',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(text)
	});
	if (!response.ok) throw new Error(await response.text());
	instructions = await response.json();
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
		fontSize: textSize('code'),
		scrollback
	};
}
