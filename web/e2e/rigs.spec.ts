import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { expect, test } from './fixtures.ts';
import { openProject } from './shell.ts';

test.describe('starter rigs', () => {
	test.use({ starterRigs: true });

	for (const [name, model, effort, mode] of [
		['Claude Code', 'opus', 'xhigh', 'auto'],
		['Codex', 'gpt-6.1-sol', 'low', 'approve-for-me']
	]) {
		test(`${name} fills in its actual starting settings`, async ({ page, request }) => {
			await page.goto('/');
			await page.getByRole('button', { name: 'Settings', exact: true }).click();
			const dialog = page.getByRole('dialog', { name: 'Settings', exact: true });
			await dialog.getByRole('combobox', { name: 'Rig', exact: true }).selectOption(name);
			await expect(dialog.getByRole('combobox', { name: 'Model', exact: true })).toBeEnabled();
			await expect(dialog.getByRole('combobox', { name: 'Model', exact: true })).toHaveValue(model);
			await expect(dialog.getByRole('combobox', { name: 'Effort level', exact: true })).toHaveValue(effort);
			await expect(dialog.getByRole('combobox', { name: 'Permission mode', exact: true })).toHaveValue(mode);
			await expect.poll(() => dialog.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
			await dialog.getByRole('button', { name: 'Save rig', exact: true }).click();
			await expect(dialog.getByRole('status')).toHaveText('Rig saved.');
			const rigs = await (await request.get('/api/rigs')).json();
			expect(rigs.find((rig: { name: string }) => rig.name === name)).toMatchObject({ model, effort, mode });
		});
	}

	test('provider settings fill missing values and explicit rig settings take precedence', async ({ page }) => {
		const directory = `${process.env.OIKONOMOS_E2E_ROOT}/codex`;
		mkdirSync(directory, { recursive: true });
		writeFileSync(`${directory}/config.toml`, 'model = "gpt-6-luna"\nmodel_reasoning_effort = "high"\n');
		await page.goto('/');
		await page.getByRole('button', { name: 'Settings', exact: true }).click();
		const dialog = page.getByRole('dialog', { name: 'Settings', exact: true });
		const rig = dialog.getByRole('combobox', { name: 'Rig', exact: true });
		await rig.selectOption('Codex');
		const model = dialog.getByRole('combobox', { name: 'Model', exact: true });
		const effort = dialog.getByRole('combobox', { name: 'Effort level', exact: true });
		await expect(model).toHaveValue('gpt-6-luna');
		await expect(effort).toHaveValue('high');
		await dialog.getByRole('button', { name: 'Save rig', exact: true }).click();
		await expect(dialog.getByRole('status')).toHaveText('Rig saved.');
		writeFileSync(`${directory}/config.toml`, 'model = "gpt-6-astra"\nmodel_reasoning_effort = "medium"\n');
		await page.reload();
		await page.getByRole('button', { name: 'Settings', exact: true }).click();
		await rig.selectOption('Codex');
		await expect(model).toBeEnabled();
		await expect(model).toHaveValue('gpt-6-luna');
		await expect(effort).toHaveValue('high');
	});

	test('both starters can be renamed and configured through the normal editor', async ({ page, request }) => {
		await openProject(page, 'alpha');
		expect((await (await request.get('/api/rigs')).json()).map((rig: { name: string }) => rig.name)).toEqual(['Claude Code', 'Codex']);
		await page.getByRole('button', { name: 'Settings', exact: true }).click();
		const dialog = page.getByRole('dialog', { name: 'Settings', exact: true });
		const rig = dialog.getByRole('combobox', { name: 'Rig', exact: true });
		for (const [original, name, provider, model, effort, mode] of [
			['Claude Code', 'Writer', 'claude-code', 'opus', 'high', 'plan'],
			['Codex', 'Reviewer', 'codex', 'gpt-6-sol', 'xhigh', 'read-only']
		]) {
			await rig.selectOption(original);
			await expect(dialog.getByRole('combobox', { name: 'Provider', exact: true })).toHaveValue(provider);
			const models = dialog.getByRole('combobox', { name: 'Model', exact: true });
			await expect(models).toBeEnabled();
			await expect(dialog.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue(original);
			await dialog.getByRole('textbox', { name: 'Name', exact: true }).fill(name);
			await models.selectOption(model);
			await dialog.getByRole('combobox', { name: 'Effort level', exact: true }).selectOption(effort);
			await dialog.getByRole('combobox', { name: 'Permission mode', exact: true }).selectOption(mode);
			await dialog.getByRole('button', { name: 'Save rig', exact: true }).click();
			await expect(dialog.getByRole('status')).toHaveText('Rig saved.');
			await expect(rig).toHaveValue(name);
			await expect(rig.locator(`option[value="${original}"]`)).toHaveCount(0);
		}
		await expect.poll(() => dialog.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
		await page.reload();
		await page.getByRole('button', { name: 'Settings', exact: true }).click();
		await rig.selectOption('Reviewer');
		await expect(dialog.getByRole('combobox', { name: 'Model', exact: true })).toHaveValue('gpt-6-sol');
		await expect(dialog.getByRole('combobox', { name: 'Effort level', exact: true })).toHaveValue('xhigh');
		await expect(dialog.getByRole('combobox', { name: 'Permission mode', exact: true })).toHaveValue('read-only');
		await dialog.getByRole('button', { name: 'Done', exact: true }).click();
		await page.getByRole('button', { name: 'New Tab' }).click();
		await expect(page.getByRole('menuitem', { name: 'Writer', exact: true })).toBeVisible();
		await expect(page.getByRole('menuitem', { name: 'Reviewer', exact: true })).toBeVisible();
	});
});

test('named rigs can be created, selected and renamed', async ({ page }) => {
	await openProject(page, 'alpha');
	await page.getByRole('button', { name: 'Settings', exact: true }).click();
	const dialog = page.getByRole('dialog', { name: 'Settings', exact: true });
	const rig = dialog.getByRole('combobox', { name: 'Rig', exact: true });
	const name = dialog.getByRole('textbox', { name: 'Name', exact: true });
	const provider = dialog.getByRole('combobox', { name: 'Provider', exact: true });
	const model = dialog.getByRole('combobox', { name: 'Model', exact: true });
	const effort = dialog.getByRole('combobox', { name: 'Effort level', exact: true });
	const mode = dialog.getByRole('combobox', { name: 'Permission mode', exact: true });
	await expect(model).toBeEnabled();
	await expect(dialog.getByRole('button', { name: 'Save rig', exact: true })).toBeDisabled();
	await name.fill('  Reviewer  ');
	await model.selectOption('opus');
	await effort.selectOption('high');
	await mode.selectOption('plan');
	await expect.poll(() => dialog.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
	await dialog.getByRole('button', { name: 'Save rig', exact: true }).click();
	await expect(dialog.getByRole('status')).toHaveText('Rig saved.');
	const created = await rig.inputValue();
	expect(created).toBe('Reviewer');
	await expect(name).toHaveValue(created);
	await dialog.getByRole('button', { name: 'Done', exact: true }).click();
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: created, exact: true }).click();
	await expect(page.getByRole('tab', { name: new RegExp(`^${created}`) }).getByTitle('done')).toBeVisible();
	await page.reload();
	await page.getByRole('button', { name: 'Settings', exact: true }).click();
	await rig.selectOption(created);
	await expect(model).toBeEnabled();
	await expect(model).toHaveValue('opus');
	await expect(effort).toHaveValue('high');
	await expect(mode).toHaveValue('plan');
	await expect(name).toHaveValue(created);
	await name.fill('Careful Reviewer');
	await dialog.getByRole('button', { name: 'Save rig', exact: true }).click();
	await expect(dialog.getByRole('status')).toHaveText('Rig saved.');
	await expect(rig).toHaveValue('Careful Reviewer');
	await expect(rig.locator('option').filter({ hasText: /^Reviewer$/ })).toHaveCount(0);
	await dialog.getByRole('button', { name: 'Done', exact: true }).click();
	await expect(page.getByRole('tab', { name: /^Careful Reviewer/ })).toBeVisible();
	await page.reload();
	await page.getByRole('button', { name: 'Settings', exact: true }).click();
	await rig.selectOption('Careful Reviewer');
	await expect(name).toHaveValue('Careful Reviewer');
	await expect(model).toBeEnabled();
	await effort.selectOption('low');
	await dialog.getByRole('button', { name: 'Save rig', exact: true }).click();
	await expect(dialog.getByRole('status')).toHaveText('Rig saved.');
	await expect(rig).toHaveValue('Careful Reviewer');
	await rig.selectOption('Fake Codex');
	await expect(provider).toHaveValue('codex');
	await expect(model).toBeEnabled();
	await model.selectOption('gpt-6-sol');
	await effort.selectOption('xhigh');
	await mode.selectOption('read-only');
	await dialog.getByRole('button', { name: 'Save rig', exact: true }).click();
	await expect(dialog.getByRole('status')).toHaveText('Rig saved.');
	await expect(rig).toHaveValue('Fake Codex');
	const config = readFileSync(`${process.env.OIKONOMOS_E2E_ROOT}/config.toml`, 'utf8');
	expect(config).toContain('OIKONOMOS_FAKE_THREADS');
	const response = await page.request.get('/api/rigs');
	const saved = (await response.json()).find((rig: { name: string }) => rig.name === 'Fake Codex');
	expect(saved).toMatchObject({ agent: 'codex', model: 'gpt-6-sol', effort: 'xhigh', mode: 'read-only' });
	expect(saved.env).toBeUndefined();
	await provider.selectOption('claude-code');
	await expect(model).toBeEnabled();
	await expect(model).toHaveValue('');
	await expect(effort).toHaveValue('');
	await expect(mode).toHaveValue('auto');
	await expect(mode.locator('option[value="read-only"]')).toHaveCount(0);
	await model.selectOption('opus');
	await effort.selectOption('high');
	await model.selectOption('haiku');
	await expect(effort).toHaveValue('');
	await expect.poll(() => dialog.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
	await page.screenshot({ path: test.info().outputPath('rig-editor.png') });
});

test('provider discovery failures can be retried and save failures keep the draft', async ({ page }) => {
	await page.route('**/api/rigs/options?*', (route) => route.fulfill({ status: 502, body: 'Provider unavailable' }));
	await page.goto('/');
	await page.getByRole('button', { name: 'Settings', exact: true }).click();
	const dialog = page.getByRole('dialog', { name: 'Settings', exact: true });
	await expect(dialog.getByRole('alert')).toHaveText('Provider unavailable');
	await expect(dialog.getByRole('button', { name: 'Save rig', exact: true })).toBeDisabled();
	await page.unroute('**/api/rigs/options?*');
	await dialog.getByRole('button', { name: 'Reload choices', exact: true }).click();
	const model = dialog.getByRole('combobox', { name: 'Model', exact: true });
	await expect(model).toBeEnabled();
	const name = dialog.getByRole('textbox', { name: 'Name', exact: true });
	await name.fill('Reviewer');
	await model.selectOption('opus');
	await page.route('**/api/rigs', (route) => route.request().method() === 'PUT' ? route.fulfill({ status: 400, body: 'Save failed' }) : route.continue());
	await dialog.getByRole('button', { name: 'Save rig', exact: true }).click();
	await expect(dialog.getByRole('alert')).toHaveText('Save failed');
	await expect(model).toHaveValue('opus');
	await expect(name).toHaveValue('Reviewer');
});

test('rig names must be nonempty and unique', async ({ page }) => {
	await page.goto('/');
	await page.getByRole('button', { name: 'Settings', exact: true }).click();
	const dialog = page.getByRole('dialog', { name: 'Settings', exact: true });
	const name = dialog.getByRole('textbox', { name: 'Name', exact: true });
	const save = dialog.getByRole('button', { name: 'Save rig', exact: true });
	await expect(dialog.getByRole('combobox', { name: 'Model', exact: true })).toBeEnabled();
	await name.fill('   ');
	await expect(save).toBeDisabled();
	await name.fill('Fake Codex');
	await save.click();
	await expect(dialog.getByRole('alert')).toHaveText('A Rig with this name already exists');
	await expect(name).toHaveValue('Fake Codex');
	await dialog.getByRole('combobox', { name: 'Rig', exact: true }).selectOption('Fake Claude');
	await name.fill('Fake Codex');
	await expect(save).toBeEnabled();
	await save.click();
	await expect(dialog.getByRole('alert')).toHaveText('A Rig with this name already exists');
});

test('rig deletion can be cancelled and retried while preserving existing chats', async ({ page, request }) => {
	const configured: { name: string }[] = await (await request.get('/api/rigs')).json();
	await openProject(page, 'alpha');
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude', exact: true }).click();
	const chat = page.getByRole('tab', { name: /^Fake Claude/ });
	await expect(chat.getByTitle('done')).toBeVisible();
	await page.getByRole('button', { name: 'Settings', exact: true }).click();
	const settings = page.getByRole('dialog', { name: 'Settings', exact: true });
	const rig = settings.getByRole('combobox', { name: 'Rig', exact: true });
	await expect(settings.getByRole('button', { name: 'Delete rig', exact: true })).toHaveCount(0);
	await rig.selectOption('Fake Claude');
	await settings.getByRole('textbox', { name: 'Name', exact: true }).fill('Unsaved name');
	await settings.getByRole('button', { name: 'Delete rig', exact: true }).click();
	const confirmation = page.getByRole('dialog', { name: 'Delete rig?', exact: true });
	await expect(confirmation).toContainText('Delete “Fake Claude”? Existing chats will be kept.');
	await expect.poll(() => confirmation.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
	await confirmation.getByRole('button', { name: 'Cancel', exact: true }).click();
	await expect(rig).toHaveValue('Fake Claude');
	await page.route('**/api/rigs', (route) => route.request().method() === 'DELETE' ? route.fulfill({ status: 400, body: 'Delete failed' }) : route.continue());
	await settings.getByRole('button', { name: 'Delete rig', exact: true }).click();
	await confirmation.getByRole('button', { name: 'Delete', exact: true }).click();
	await expect(confirmation.getByRole('alert')).toHaveText('Delete failed');
	await expect(confirmation.getByRole('button', { name: 'Delete', exact: true })).toBeEnabled();
	await page.unroute('**/api/rigs');
	await confirmation.getByRole('button', { name: 'Delete', exact: true }).click();
	await expect(confirmation).toBeHidden();
	await expect(rig).toHaveValue('');
	await expect(rig.locator('option[value="Fake Claude"]')).toHaveCount(0);
	await expect(settings.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue('');
	expect(await (await request.get('/api/rigs')).json()).toEqual(configured.filter((rig) => rig.name !== 'Fake Claude'));
	await settings.getByRole('button', { name: 'Done', exact: true }).click();
	await expect(chat).toBeVisible();
	await page.getByRole('button', { name: 'New Tab' }).click();
	await expect(page.getByRole('menuitem', { name: 'Fake Claude', exact: true })).toHaveCount(0);
	await expect(page.getByRole('menuitem', { name: 'Fake Codex', exact: true })).toBeVisible();
	await page.reload();
	await expect(chat).toBeVisible();
	await page.getByRole('button', { name: 'Settings', exact: true }).click();
	await expect(rig.locator('option[value="Fake Claude"]')).toHaveCount(0);
});
