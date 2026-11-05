import { afterEach, expect, test, vi } from 'vitest';

vi.stubGlobal('fetch', () => new Promise(() => {}));
const { applyTheme } = await import('#lib/config.svelte.ts');

const files = import.meta.glob<string>('../../crates/stampede/themes/*', { query: '?raw', import: 'default', eager: true });
const catalog = import.meta.glob<string>('../../crates/stampede/vendor/ghostty-themes/themes/*', { query: '?raw', import: 'default', eager: true });

type Parsed = { background: string; foreground: string; palette?: string[] };

function parse(text: string): Parsed {
	const values = Object.fromEntries(text.split('\n').filter(Boolean).map((line) => line.split(/\s*=\s*(.*)/s).slice(0, 2)));
	const palette = text.match(/^palette = \d+=#\w+$/gm)!.map((line) => line.split('=').at(-1)!);
	return { background: values.background, foreground: values.foreground, palette };
}

const palettes: Record<string, Parsed> = {
	...Object.fromEntries(Object.entries(files).map(([path, text]) => [path.split('/').at(-1)!, parse(text)])),
	'Monokai Pro Light': { background: '#faf4f2', foreground: '#29242a' }
};

test('reads all five Stampede themes', () => {
	expect(Object.keys(palettes).sort()).toEqual(['Jet', 'Limestone', 'Monokai Pro Light', 'Shale', 'Vellum', 'Walnut']);
	for (const { palette } of Object.values(palettes)) if (palette) expect(palette).toHaveLength(16);
});

const canvas = document.createElement('canvas').getContext('2d', { willReadFrequently: true })!;

function rendered(color: string): number[] {
	canvas.clearRect(0, 0, 1, 1);
	canvas.fillStyle = color;
	canvas.fillRect(0, 0, 1, 1);
	return [...canvas.getImageData(0, 0, 1, 1).data.slice(0, 3)];
}

function luminance(rgb: number[]): number {
	const [r, g, b] = rgb.map((value) => {
		const c = value / 255;
		return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
	});
	return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function contrast(a: number[], b: number[]): number {
	const [light, dark] = [luminance(a), luminance(b)].sort((x, y) => y - x);
	return (light + 0.05) / (dark + 0.05);
}

afterEach(() => {
	delete document.documentElement.dataset.scheme;
	document.body.replaceChildren();
});

function measure(name: string, { background, foreground, palette: colors }: Parsed, scheme: 'light' | 'dark') {
	const palette = {
		name,
		background,
		foreground,
		palette: colors ?? Array(16).fill(foreground),
		cursor: foreground,
		cursorText: background,
		selectionBackground: foreground,
		selectionForeground: background
	};
	applyTheme({ name, light: palette, dark: palette });
	document.documentElement.dataset.scheme = scheme;
	const probes = ['--color-bg', '--surface-1'].map((surface) => {
		const probe = document.createElement('p');
		probe.textContent = 'Muted';
		probe.style.cssText = `background: var(${surface}); color: var(--muted-fg)`;
		document.body.append(probe);
		return probe;
	});
	const [page, surface] = probes.map((probe) => rendered(getComputedStyle(probe).backgroundColor));
	const muted = rendered(getComputedStyle(probes[0]).color);
	probes.forEach((probe) => probe.remove());
	return { page, surface, muted };
}

for (const [name, parsed] of Object.entries(palettes)) {
	for (const scheme of ['light', 'dark'] as const) {
		test(`${name} muted text reads at 4.5:1 on the background and first surface in ${scheme}`, () => {
			const { page, surface, muted } = measure(name, parsed, scheme);
			expect(page).toEqual(rendered(parsed.background));
			expect(surface).not.toEqual(page);
			expect(contrast(muted, page)).toBeGreaterThanOrEqual(4.5);
			expect(contrast(muted, surface)).toBeGreaterThanOrEqual(4.5);
		});
	}
}

for (const scheme of ['light', 'dark'] as const) {
	test(`every catalog theme's muted text reads at 4.5:1 on the background and first surface in ${scheme}`, () => {
		const failing = Object.entries(catalog).flatMap(([path, text]) => {
			const name = path.split('/').at(-1)!;
			const { page, surface, muted } = measure(name, parse(text), scheme);
			const worst = Math.min(contrast(muted, page), contrast(muted, surface));
			return worst < 4.5 ? [`${name} ${worst.toFixed(2)}`] : [];
		});
		expect(Object.keys(catalog).length).toBeGreaterThan(400);
		expect(failing).toEqual([]);
	});
}
