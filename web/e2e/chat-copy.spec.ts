import type { Item, ServerMessage } from '#lib/device.svelte.ts';
import { expect, test } from './fixtures.ts';
import { mkdirSync } from 'node:fs';
import { openProject } from './shell.ts';

test('chat copies code blocks and long inline code', { tag: '@desktop' }, async ({ page, context }, { project, repeatEachIndex }) => {
	await context.grantPermissions(['clipboard-read', 'clipboard-write']);
	let chatId: string;
	const items: Item[] = [{ kind: 'text', text: 'Run `ls` or `pnpm install --frozen-lockfile`:\n\n```sh\necho one\necho two\n```', streaming: false, phase: 'final' }];
	await page.routeWebSocket('**/ws', (socket) => {
		const server = socket.connectToServer();
		server.onMessage((data) => {
			if (typeof data !== 'string') return socket.send(data);
			const message: ServerMessage = JSON.parse(data);
			if (message.type === 'chat.created') chatId = message.chat.id;
			socket.send(message.type === 'chat.items' && message.chatId === chatId ? JSON.stringify({ ...message, items }) : data);
		});
	});
	const name = `chat-copy-${project.name}-${repeatEachIndex}`;
	mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`);
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude', exact: true }).click();
	const text = page.locator('.panel[role="tabpanel"]:visible .text');
	const copies = text.getByRole('button', { name: 'Copy' });
	await expect(copies).toHaveCount(2);
	await expect(text.locator('.copy svg:visible')).toHaveCount(2);

	await copies.first().click();
	await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe('pnpm install --frozen-lockfile');
	await copies.last().click();
	await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe('echo one\necho two');
	await expect(copies.last()).toHaveAttribute('data-copied');
	await expect(copies.last()).not.toHaveAttribute('data-copied');
});
