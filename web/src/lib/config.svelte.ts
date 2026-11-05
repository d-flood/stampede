import type { ITheme } from 'ghostty-web';
import { isDark } from '#lib/scheme.svelte.ts';
import { loadSidebarSide } from '#lib/sidebar.svelte.ts';
import { loadTextSizes, textSize } from '#lib/typography.svelte.ts';

export type Palette = {
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
export { agentNames } from '#lib/chat/sessions.ts';
export type Rig = { name: string; agent: Agent; shortcut?: string; model?: string; effort?: string; speed?: string; ultracode?: string; mode?: string };
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

function fetchConfig() {
	return Promise.all([
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
}

// The first load starts with this module, so its requests overlap the download of the page code.
let startup: ReturnType<typeof fetchConfig> | undefined = fetchConfig();

export async function loadConfig() {
	loadTextSizes();
	loadSidebarSide();
	const responses = startup ?? fetchConfig();
	startup = undefined;
	const [name, theme, font, ui, chat, code, rigs, settings, leaderKey, keys, lines, listed, commitRigName, commitText, agentModes, saved, autoFetching] = await responses;
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

function linear(hex: string): number[] {
	return [1, 3, 5].map((i) => {
		const c = parseInt(hex.slice(i, i + 2), 16) / 255;
		return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
	});
}

function luminance(hex: string): number {
	const [r, g, b] = linear(hex);
	return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function oklab(hex: string): number[] {
	const [r, g, b] = linear(hex);
	const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
	const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
	const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
	return [
		0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
		1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
		0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s
	];
}

// Matches CSS color-mix(in oklab, a weight, b).
function mix(a: string, b: string, weight: number): string {
	const [x, y] = [oklab(a), oklab(b)];
	const [L, A, B] = x.map((value, i) => value * weight + y[i] * (1 - weight));
	const l = (L + 0.3963377774 * A + 0.2158037573 * B) ** 3;
	const m = (L - 0.1055613458 * A - 0.0638541728 * B) ** 3;
	const s = (L - 0.0894841775 * A - 1.291485548 * B) ** 3;
	const rgb = [
		4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
		-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
		-0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s
	];
	return hex(rgb);
}

function hex(rgb: number[]): string {
	return `#${rgb
		.map((c) => {
			const encoded = c <= 0.0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055;
			return Math.round(Math.min(1, Math.max(0, encoded)) * 255).toString(16).padStart(2, '0');
		})
		.join('')}`;
}

const d50 = [0.96429567, 1, 0.8251046];
const toXyz = [
	[0.436065742824811, 0.3851514688337912, 0.14307845442264197],
	[0.22249319175623702, 0.7168870538238823, 0.06061979053616537],
	[0.013923904500943465, 0.09708128566574634, 0.7140993584005155]
];
const fromXyz = [
	[3.1341359569958707, -1.6173863321612538, -0.4906619460083532],
	[-0.978795502912089, 1.916254567259524, 0.03344273116131949],
	[0.07195537988411677, -0.2289768264158322, 1.405386058324125]
];

function lab(hex: string): number[] {
	const rgb = linear(hex);
	const [x, y, z] = toXyz.map((row, i) => {
		const t = row.reduce((sum, value, j) => sum + value * rgb[j], 0) / d50[i];
		return t > 216 / 24389 ? Math.cbrt(t) : ((24389 / 27) * t + 16) / 116;
	});
	return [116 * y - 16, 500 * (x - y), 200 * (y - z)];
}

// Matches CSS color-mix(in lab, a weight, b).
function mixLab(a: string, b: string, weight: number): string {
	const [x, y] = [lab(a), lab(b)];
	const [L, A, B] = x.map((value, i) => value * weight + y[i] * (1 - weight));
	const fy = (L + 16) / 116;
	const xyz = [fy + A / 500, fy, fy - B / 200].map((f, i) => (f ** 3 > 216 / 24389 ? f ** 3 : (116 * f - 16) / (24389 / 27)) * d50[i]);
	return hex(fromXyz.map((row) => row.reduce((sum, value, j) => sum + value * xyz[j], 0)));
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

function mutedText({ foreground, background }: Palette): string {
	const surface = mixLab(background, foreground, 0.96);
	for (let percent = 60; percent < 100; percent++) {
		const muted = mix(foreground, background, percent / 100);
		if (contrast(muted, background) >= 4.5 && contrast(muted, surface) >= 4.5) return muted;
	}
	return foreground;
}

export function applyTheme(theme: ThemeResponse) {
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
		style.setProperty(`--${scheme}-muted-fg`, mutedText(palette));
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

export async function saveRig(settings: { original?: string; name: string; agent: Agent; model?: string; effort?: string; speed?: string; ultracode?: string; mode: string }) {
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
