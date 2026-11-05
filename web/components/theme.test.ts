import { afterEach, expect, test, vi } from 'vitest';

vi.stubGlobal('fetch', () => new Promise(() => {}));
const { applyTheme } = await import('#lib/config.svelte.ts');

const palettes: Record<string, { background: string; foreground: string }> = {
	Default: { background: '#282c34', foreground: '#ffffff' },
	'Monokai Pro Light': { background: '#faf4f2', foreground: '#29242a' }
};

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

for (const [name, { background, foreground }] of Object.entries(palettes)) {
	for (const scheme of ['light', 'dark'] as const) {
		test(`${name} muted text reads at 4.5:1 on the background and first surface in ${scheme}`, () => {
			const palette = {
				name,
				background,
				foreground,
				palette: Array(16).fill(foreground),
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
			expect(page).toEqual(rendered(background));
			expect(surface).not.toEqual(page);
			expect(contrast(muted, page)).toBeGreaterThanOrEqual(4.5);
			expect(contrast(muted, surface)).toBeGreaterThanOrEqual(4.5);
		});
	}
}
