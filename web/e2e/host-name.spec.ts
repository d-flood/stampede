import { openSettings, showScreen } from './shell.ts';
import { expect, test } from './fixtures.ts';
import { readFileSync, writeFileSync } from 'node:fs';

test('the Host name titles every page, heads the phone Projects screen, and saves into the config', async ({ page }, { project }) => {
	const phone = project.name === 'pixel';
	const config = `${process.env.STAMPEDE_E2E_ROOT}/config.toml`;
	const original = readFileSync(config, 'utf8');
	const heading = page.getByRole('heading', { level: 1 });
	const settings = page.getByRole('dialog', { name: 'Settings', exact: true });
	const section = settings.getByRole('region', { name: 'Host name', exact: true });
	const field = section.getByRole('textbox', { name: 'Host name', exact: true });
	const commit = () => (phone ? field.blur() : field.press('Enter'));
	const done = () => settings.getByRole('button', { name: 'Done', exact: true }).click();

	await page.goto('/');
	if (phone) {
		await showScreen(page, 'projects');
		await expect(heading).toHaveText('Projects');
		await expect(heading.locator('.sub')).toHaveCount(0);
	}
	await expect(page).toHaveTitle('Stampede');

	await openSettings(page, 'General');
	await expect(field).toHaveValue('');
	await expect(field).toHaveAttribute('placeholder', 'Stampede');
	await field.fill('atlas');
	await commit();
	await expect(page).toHaveTitle('atlas');
	expect(readFileSync(config, 'utf8')).toBe(`name = "atlas"\n${original}`);
	await done();
	if (phone) await expect(heading).toHaveText('Projects atlas');

	const other = await page.context().newPage();
	await other.goto('/');
	await expect(other).toHaveTitle('atlas');
	const loaded = await other.evaluate(() => performance.timeOrigin);
	await openSettings(page, 'General');
	await expect(field).toHaveValue('atlas');
	await field.fill('zeus');
	await commit();
	await expect(page).toHaveTitle('zeus');
	await expect(other).toHaveTitle('zeus');
	expect(await other.evaluate(() => performance.timeOrigin)).toBe(loaded);

	const invalid = '[editor\n';
	writeFileSync(config, invalid);
	await field.fill('broken');
	await commit();
	await expect(section.getByRole('alert')).not.toBeEmpty();
	expect(readFileSync(config, 'utf8')).toBe(invalid);
	await expect(page).toHaveTitle('zeus');

	writeFileSync(config, `name = "zeus"\n${original}`);
	await field.fill('');
	await commit();
	await expect(page).toHaveTitle('Stampede');
	await expect(section.getByRole('alert')).toHaveCount(0);
	expect(readFileSync(config, 'utf8')).toBe(original);
	await done();
	if (phone) {
		await expect(heading).toHaveText('Projects');
		await expect(heading.locator('.sub')).toHaveCount(0);
	}
	await expect(other).toHaveTitle('Stampede');
});
