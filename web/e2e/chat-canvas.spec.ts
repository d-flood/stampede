import type { ServerMessage } from '#lib/device.svelte.ts';
import { expect, test } from './fixtures.ts';
import { mkdirSync, writeFileSync } from 'node:fs';
import { closeProject, openProject } from './shell.ts';

test('a chat canvas opens beside its chat, follows edits, and keeps versions', async ({ page }, { project, repeatEachIndex }) => {
	let chatId = '';
	await page.routeWebSocket('**/ws', (socket) => {
		const server = socket.connectToServer();
		server.onMessage((data) => {
			if (typeof data !== 'string') return socket.send(data);
			const message: ServerMessage = JSON.parse(data);
			if (message.type === 'chat.created') chatId = message.chat.id;
			if (message.type === 'chat.items' && message.chatId === chatId) {
				socket.send(JSON.stringify({ ...message, items: [{ kind: 'canvas', canvas: `${chatId}/report.html`, added: 1, removed: 0, write: true }] }));
			} else socket.send(data);
		});
	});
	const name = `canvas-${project.name}-${repeatEachIndex}`;
	mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude', exact: true }).click();
	const card = page.getByText('Wrote canvas report');
	await expect(card).toBeVisible();
	const file = `${process.env.STAMPEDE_E2E_ROOT}/data/canvases/${chatId}/report.html`;
	writeFileSync(file, '<h1>First draft</h1>');
	const canvas = page.getByRole('region', { name: 'report' });
	const content = canvas.locator('iframe').contentFrame();
	const tab = page.getByRole('tab', { name: 'report' });
	if (project.name === 'desktop') {
		await expect(content.getByRole('heading', { name: 'First draft' })).toBeVisible();
		await expect(page.getByRole('tablist', { name: 'Right Tabs' }).getByRole('tab', { name: 'report' })).toHaveAttribute('aria-selected', 'true');
		await expect(card).toBeVisible();
	} else {
		await expect(tab).toHaveAttribute('aria-selected', 'false');
		await page.getByRole('button', { name: 'Open', exact: true }).click();
		await expect(content.getByRole('heading', { name: 'First draft' })).toBeVisible();
	}
	await expect(canvas.locator('iframe')).toHaveAttribute('sandbox', 'allow-scripts');
	const long = (heading: string) => `<h1>${heading}</h1><div style="height:3000px"></div>`;
	writeFileSync(file, long('Second draft'));
	await expect(content.getByRole('heading', { name: 'Second draft' })).toBeVisible();
	await expect(canvas.getByText('v2 of 2')).toBeVisible();
	await content.locator('html').evaluate(() => scrollTo(0, 1200));
	writeFileSync(file, long('Third draft'));
	await expect(canvas.getByText('v3 of 3')).toBeVisible();
	await expect(content.getByRole('heading', { name: 'Third draft' })).toBeAttached();
	await expect.poll(() => content.locator('html').evaluate(() => scrollY)).toBe(1200);
	await canvas.getByRole('button', { name: 'Previous version' }).click();
	await expect(content.getByRole('heading', { name: 'Second draft' })).toBeAttached();
	writeFileSync(file, long('Fourth draft'));
	await expect(canvas.getByText('v2 of 4')).toBeVisible();
	await expect(content.getByRole('heading', { name: 'Second draft' })).toBeAttached();
	await canvas.getByRole('button', { name: 'Next version' }).click();
	await canvas.getByRole('button', { name: 'Next version' }).click();
	await expect(content.getByRole('heading', { name: 'Fourth draft' })).toBeAttached();
	await expect(canvas.getByRole('button', { name: 'Next version' })).toBeDisabled();
	await page.reload();
	await expect(content.getByRole('heading', { name: 'Fourth draft' })).toBeAttached();
	await page.getByRole('button', { name: 'Close report' }).click();
	await expect(tab).not.toBeVisible();
	await page.getByRole('region', { name: 'Canvases' }).getByRole('button', { name: 'report' }).click();
	await expect(content.getByRole('heading', { name: 'Fourth draft' })).toBeAttached();
	await expect(tab).toHaveAttribute('aria-selected', 'true');
	await closeProject(page, name);
});
