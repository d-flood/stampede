import { mkdirSync } from 'node:fs';
import { expect, test } from './fixtures.ts';
import { closeProject, openProject } from './shell.ts';

test('chat composer submits with Enter on desktop and keeps mobile Enter as a newline', async ({ page, context }, { project }) => {
	const sent: { text: string; queue: boolean }[] = [];
	page.on('websocket', (socket) => socket.on('framesent', ({ payload }) => {
		if (typeof payload !== 'string') return;
		const message = JSON.parse(payload);
		if (message.type === 'chat.send') sent.push(message);
	}));
	const name = `composer-${project.name}`;
	mkdirSync(`${process.env.OIKONOMOS_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab', exact: true }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude', exact: true }).click();
	await expect(page.getByRole('tab', { name: /^Fake Claude/ }).getByTitle('done')).toBeVisible();
	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	const composer = panel.getByRole('textbox', { name: 'Composer' });
	await composer.fill('/eff');
	await composer.press('Enter');
	await expect(composer).toHaveValue('/effort ');
	expect(sent).toHaveLength(0);
	await composer.fill('First line');
	await composer.dispatchEvent('keydown', { key: 'Enter', isComposing: true });
	await expect(composer).toHaveValue('First line');
	expect(sent).toHaveLength(0);
	await composer.press('Shift+Enter');
	await composer.press('s');
	await expect(composer).toHaveValue('First line\ns');
	expect(sent).toHaveLength(0);
	await composer.press('Enter');
	if (project.name === 'pixel') {
		await expect(composer).toHaveValue('First line\ns\n');
		expect(sent).toHaveLength(0);
		await panel.getByRole('button', { name: 'Send', exact: true }).click();
	}
	await expect(composer).toHaveValue('');
	await expect.poll(() => sent.map(({ text, queue }) => ({ text, queue }))).toEqual([
		{ text: `First line\ns${project.name === 'pixel' ? '\n' : ''}`, queue: false }
	]);
	if (project.name === 'desktop') {
		await composer.press('Enter');
		await expect(composer).toHaveValue('');
		await context.setOffline(true);
		await composer.fill('Offline draft');
		await expect(panel.getByRole('button', { name: /^(Send|Steer)$/ })).toBeDisabled();
		await composer.press('Enter');
		await expect(composer).toHaveValue('Offline draft');
		expect(sent).toHaveLength(1);
		await context.setOffline(false);
	}
	await closeProject(page, name);
});
