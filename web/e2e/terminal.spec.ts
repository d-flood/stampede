import { expect, test } from './fixtures.ts';
import { expectShown, newTab, openProject, screen, showScreen } from './shell.ts';
import { existsSync, writeFileSync } from 'node:fs';

test('a hidden Terminal displays output received while another Tab was active', { tag: '@desktop' }, async ({ page }) => {
	await openProject(page, 'alpha');
	const tabs = page.getByRole('tablist', { name: 'Tabs' }).getByRole('tab');
	for (let index = 0; index < 2; index++) {
		await newTab(page, 'Terminal');
		await expect(tabs).toHaveCount(index + 1);
		await expect.poll(() => screen(page)).toMatch(/\S/);
		if (index === 0) {
			await page.keyboard.type('while test ! -f release-output; do sleep 0.02; done; seq 1 40; touch output-done');
			await page.keyboard.press('Enter');
		}
	}
	await expect(tabs.first()).toHaveAttribute('aria-selected', 'false');
	const root = `${process.env.STAMPEDE_E2E_ROOT}/projects/alpha`;
	writeFileSync(`${root}/release-output`, '');
	await expect.poll(() => existsSync(`${root}/output-done`)).toBe(true);
	await tabs.first().click();
	await expect.poll(() => screen(page)).toMatch(/^40$/m);
});

test('a Terminal runs echo hi and a deep link reveals its Tab', async ({ page }, { project }) => {
	let target: { id: string; owner: { project: string } } | undefined;
	page.on('websocket', (socket) => socket.on('framereceived', ({ payload }) => {
		if (typeof payload !== 'string') return;
		const message = JSON.parse(payload);
		if (message.type === 'terminal.created') target = message.terminal;
	}));
	const fixture = project.name === 'desktop' ? 'alpha' : 'beta';
	await openProject(page, fixture);
	await newTab(page, 'Terminal');
	await expect.poll(() => screen(page), { timeout: 15_000 }).toMatch(/\S/);
	await page.keyboard.type('echo hi');
	await page.keyboard.press('Enter');
	await expect.poll(() => screen(page)).toMatch(/^hi$/m);
	await expect.poll(() => target).toBeDefined();
	await expectShown(page, target!.id);
	if (project.name === 'pixel') {
		await showScreen(page, 'projects');
		await page.getByRole('button', { name: 'Drawer', exact: true }).click();
	} else await page.keyboard.press('Control+Backquote');
	const drawer = page.getByRole('region', { name: 'Drawer', includeHidden: true });
	await expect.poll(() => drawer.evaluate((element) => getComputedStyle(element).transform)).toBe('none');
	expect(target).toBeDefined();
	await page.goto(`/?project=${target!.owner.project}&tab=${target!.id}`);
	await expect(drawer).toHaveAttribute('inert', '');
	await expect.poll(() => drawer.evaluate((element) => element.getBoundingClientRect().bottom)).toBeLessThanOrEqual(0);
	await expectShown(page, target!.id);
});
