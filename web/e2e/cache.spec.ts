import type { ServerMessage } from '#lib/device.svelte.ts';
import { expect, test } from './fixtures.ts';
import { mkdirSync } from 'node:fs';
import { newTab, openProject, pinned } from './shell.ts';

test('a cold prompt cache turns the context ring red and confirms before re-reading a large context', async ({ page }, { project, repeatEachIndex }) => {
	let chatId = '';
	const expired = Date.now() - 3 * 24 * 60 * 60 * 1000;
	await page.routeWebSocket('**/ws', (socket) => {
		const server = socket.connectToServer();
		server.onMessage((data) => {
			if (typeof data !== 'string') return socket.send(data);
			const message: ServerMessage = JSON.parse(data);
			if (message.type === 'chat.created') chatId = message.chat.id;
			if (message.type === 'chat.meters' && message.chatId === chatId && message.context) {
				socket.send(JSON.stringify({ ...message, context: { used: 512_000, window: 1_000_000, cache: { expires: expired, then: 'cold' } } }));
			} else socket.send(data);
		});
	});
	const name = `cache-${project.name}-${repeatEachIndex}`;
	mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await newTab(page, 'Fake Claude');
	const tab = await pinned(page.getByRole('tab', { name: /^Fake Claude/ }));
	await expect(tab.getByTitle('done')).toBeVisible();

	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	const composer = panel.getByRole('textbox', { name: 'Composer' });
	const send = panel.getByRole('button', { name: 'Send' });
	await composer.fill('Greet the reader');
	await send.click();
	await expect(panel.getByText(/^Hello and welcome!/)).toBeVisible();
	await expect(tab.getByTitle('done')).toBeVisible();
	const context = panel.getByRole('meter', { name: 'Context' });
	await expect(context).toHaveAttribute('aria-valuetext', '512k / 1M (51%), cache cold');
	if (project.name === 'desktop') {
		await context.hover();
		await expect(page.getByRole('tooltip').filter({ hasText: 'Context 512k / 1M (51%)' })).toContainText('Cache cold · expired 3 days ago');
	}

	await composer.fill('Anything else?');
	await send.click();
	const dialog = page.getByRole('dialog', { name: 'Cache cold' });
	await expect(dialog).toContainText('The prompt cache expired 3 days ago, so this message re-reads all 512k tokens of context uncached.');
	await dialog.getByRole('button', { name: 'Cancel' }).click();
	await expect(dialog).toBeHidden();
	await expect(composer).toHaveValue('Anything else?');
	await expect(composer).toBeFocused();

	await send.click();
	await dialog.getByRole('button', { name: 'Send anyway' }).click();
	await expect(dialog).toBeHidden();
	await expect(composer).toHaveValue('');
	await expect(panel.getByRole('region', { name: 'User message 2' }).getByText('Anything else?')).toBeVisible();
	await expect(tab.getByTitle('working')).toBeVisible();
});
