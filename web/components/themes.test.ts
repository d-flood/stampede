import { selectTheme } from '#lib/config.svelte.ts';
import ThemePicker from '#lib/shell/ThemePicker.svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

const backgrounds: Record<string, string> = {
	Base: '#101010',
	Alabaster: '#f7f7f7',
	Dracula: '#282a36',
	'Dracula+': '#212121',
	Dawnfox: '#faf4ed',
	'3024 Day': '#f7f7f7',
	Vesper: '#101010',
	'Zenbones Dark': '#1c1917'
};
const names = Object.keys(backgrounds).filter((name) => name !== 'Base').sort();
const rgb = (hex: string) => `rgb(${[1, 3, 5].map((at) => parseInt(hex.slice(at, at + 2), 16)).join(', ')})`;
const background = () => getComputedStyle(document.body).backgroundColor;

function theme(name: string) {
	const palette = { name, background: backgrounds[name], foreground: '#808080', palette: [], cursor: '#808080', cursorText: '#000000', selectionBackground: '#404040', selectionForeground: '#ffffff' };
	return Response.json({ name, light: palette, dark: palette });
}

let overrides: Map<string, () => Response | Promise<Response>>;

beforeEach(async () => {
	overrides = new Map();
	vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
		const path = String(input);
		const override = overrides.get(path);
		if (override) return override();
		if (path === '/api/themes') return Response.json(names);
		if (path === '/api/theme') return theme('Base');
		return theme(decodeURIComponent(path.replace('/api/themes/', '')));
	});
	await selectTheme('Base');
});

async function open() {
	const onselect = vi.fn();
	const view = await render(ThemePicker, { onselect, onpreview: vi.fn() });
	const option = (name: string) => view.getByRole('option', { name, exact: true });
	const search = view.getByRole('combobox', { name: 'Search themes' });
	const apply = view.getByRole('button', { name: 'Apply theme', exact: true });
	const choose = async (name: string) => {
		await search.fill(name);
		await option(name).click();
	};
	return { view, onselect, option, search, apply, choose, all: view.getByRole('checkbox', { name: 'Show all themes' }) };
}

function gated(name: string) {
	let release!: () => void;
	const gate = new Promise<void>((resolve) => { release = resolve; });
	let answered!: () => void;
	const finished = new Promise<void>((resolve) => { answered = resolve; });
	overrides.set(`/api/themes/${name}`, async () => {
		await gate;
		const body = await theme(name).json();
		return { ok: true, json: async () => { setTimeout(answered); return body; } } as Response;
	});
	return { release, finished };
}

test('late previews cannot replace a newer preview or restore after closing', async () => {
	const { view, all, apply, choose } = await open();
	await all.click();
	const newer = gated('Dracula');
	await choose('Dracula');
	await expect.element(apply).toBeDisabled();
	await choose('Alabaster');
	await expect.element(apply).toBeEnabled();
	newer.release();
	await newer.finished;
	expect(background()).toBe(rgb(backgrounds.Alabaster));

	const closed = gated('Dracula');
	await choose('Dracula');
	await view.unmount();
	await expect.poll(background).toBe(rgb(backgrounds.Base));
	closed.release();
	await closed.finished;
	expect(background()).toBe(rgb(backgrounds.Base));
});

test('preview and save failures leave the saved theme intact', async () => {
	const { view, all, apply, choose, onselect } = await open();
	await all.click();
	overrides.set('/api/themes/Dracula', () => new Response('Invalid theme', { status: 400 }));
	await choose('Dracula');
	await expect.element(view.getByRole('alert')).toBeVisible();
	await expect.element(apply).toBeDisabled();
	await expect.poll(background).toBe(rgb(backgrounds.Base));
	await choose('Alabaster');
	await expect.element(apply).toBeEnabled();
	overrides.set('/api/theme', () => new Response('Could not save theme.', { status: 500 }));
	await apply.click();
	await expect.element(view.getByRole('alert')).toHaveTextContent('Could not save theme.');
	expect(onselect).not.toHaveBeenCalled();
	await view.unmount();
	await expect.poll(background).toBe(rgb(backgrounds.Base));
});

test('the picker shortlists light and dark themes until all are shown', async () => {
	const { view, all, option, search } = await open();
	const preview = () => view.container.querySelector('.preview strong')?.textContent;
	await view.getByRole('button', { name: /^Dark/ }).click();
	await expect.element(search).toHaveAttribute('placeholder', 'Search dark themes');
	await expect.element(option('Vesper')).toBeVisible();
	await expect.element(option('Dawnfox')).not.toBeInTheDocument();
	await expect.element(option('Dracula')).not.toBeInTheDocument();
	await view.getByRole('button', { name: 'Previous theme', exact: true }).click();
	await expect.poll(preview).toBe('Zenbones Dark');
	await view.getByRole('button', { name: /^Light/ }).click();
	await expect.element(search).toHaveAttribute('placeholder', 'Search light themes');
	await expect.element(option('Dawnfox')).toBeVisible();
	await expect.element(option('Vesper')).not.toBeInTheDocument();
	await view.getByRole('button', { name: 'Next theme', exact: true }).click();
	await expect.poll(preview).toBe('3024 Day');
	await all.click();
	await expect.element(search).toHaveAttribute('placeholder', 'Search all themes');
	await expect.element(option('Dracula')).toBeVisible();
	await expect.element(option('Vesper')).toBeVisible();
	await expect.element(option('Dawnfox')).toBeVisible();
	await expect.poll(() => document.documentElement.scrollWidth <= window.innerWidth).toBe(true);
});
