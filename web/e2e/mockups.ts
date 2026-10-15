import { devices, expect, test, type Browser, type BrowserContextOptions } from '@playwright/test';
import { mkdir, writeFile } from 'node:fs/promises';

const out = new URL('../../docs/mockups/', import.meta.url);
const desktop: BrowserContextOptions = { viewport: { width: 2560, height: 1440 } };
const phone: BrowserContextOptions = {
	...devices['Pixel 8 Pro'],
	viewport: { width: 448, height: 998 },
	deviceScaleFactor: 2
};
const screens = ['Projects', 'Tabs', 'Sidebar'];
const gap = 24;

test.describe.configure({ mode: 'serial' });

async function open(browser: Browser, options: BrowserContextOptions, n: number) {
	const context = await browser.newContext(options);
	const page = await context.newPage();
	await page.goto(`/dev/mockups/${n}`);
	await expect(page.getByRole('tablist').first()).toBeVisible();
	await page.evaluate(() => document.fonts.ready);
	return { context, page };
}

async function shootDesktop(browser: Browser, n: number, colorScheme: 'dark' | 'light', file: string) {
	const { context, page } = await open(browser, { ...desktop, colorScheme }, n);
	await page.screenshot({ path: new URL(file, out).pathname, animations: 'disabled' });
	await context.close();
}

async function shootPhone(browser: Browser, n: number) {
	const { context, page } = await open(browser, { ...phone, colorScheme: 'dark' }, n);
	const frames: string[] = [];
	for (const name of screens) {
		await page.getByRole('region', { name }).evaluate((screen) => screen.scrollIntoView({ behavior: 'instant' }));
		frames.push((await page.screenshot({ animations: 'disabled' })).toString('base64'));
	}
	const { width, height } = phone.viewport!;
	const sheet = await context.browser()!.newPage({
		viewport: { width: screens.length * width + (screens.length - 1) * gap, height },
		deviceScaleFactor: phone.deviceScaleFactor
	});
	const images = frames.map((frame) => `<img width="${width}" height="${height}" src="data:image/png;base64,${frame}">`);
	await sheet.setContent(`<body style="display: flex; gap: ${gap}px; margin: 0">${images.join('')}</body>`);
	await sheet.screenshot({ path: new URL(`${n}-phone.png`, out).pathname, omitBackground: true });
	await sheet.close();
	await context.close();
}

test('dark: desktop and phone', async ({ browser }) => {
	await mkdir(out, { recursive: true });
	for (const n of [1, 2, 3]) {
		await shootDesktop(browser, n, 'dark', `${n}-desktop.png`);
		await shootPhone(browser, n);
	}
});

test('light: desktop', async ({ browser, request }) => {
	const config = new URL('config.toml', `file://${process.env.OIKONOMOS_E2E_ROOT}/`);
	await writeFile(config, 'theme = "light:One Half Light,dark:Ghostty Default Style Dark"\n');
	await expect.poll(async () => (await (await request.get('/api/theme')).json()).light.background).toBe('#fafafa');
	for (const n of [1, 2, 3]) await shootDesktop(browser, n, 'light', `${n}-desktop-light.png`);
});
