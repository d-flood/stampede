import type { Item, ServerMessage } from '#lib/device.svelte.ts';
import { expect, test } from './fixtures.ts';
import type { WebSocketRoute } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { closeProject, openProject } from './shell.ts';

test('chat follows messages until scrolled up and resumes at the bottom', { tag: '@desktop' }, async ({ page }, { project, repeatEachIndex }) => {
	let route: WebSocketRoute;
	let chatId: string;
	const items: Item[] = [{ kind: 'text', text: 'Start\n\n' + 'A line of history.\n\n'.repeat(80), streaming: false, phase: 'commentary' }];
	await page.routeWebSocket('**/ws', (socket) => {
		const server = socket.connectToServer();
		server.onMessage((data) => {
			if (typeof data !== 'string') return socket.send(data);
			const message: ServerMessage = JSON.parse(data);
			if (message.type === 'chat.created') { route = socket; chatId = message.chat.id; }
			socket.send(message.type === 'chat.items' && message.chatId === chatId ? JSON.stringify({ ...message, items }) : data);
		});
	});
	const name = `chat-scroll-${project.name}-${repeatEachIndex}`;
	mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`);
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude', exact: true }).click();
	const panel = page.locator('.panel[role="tabpanel"]:visible');
	const transcript = panel.locator('.transcript:visible');
	const distance = () => transcript.evaluate((node) => node.scrollHeight - node.scrollTop - node.clientHeight);
	await expect(panel.getByText('Start', { exact: true })).toBeAttached();
	await expect.poll(distance).toBeLessThan(2);

	async function append(index: number) {
		const item: Item = { kind: 'text', text: `Message ${index}\n\n` + 'New streaming line.\n\n'.repeat(20), streaming: true, phase: 'commentary' };
		route!.send(JSON.stringify({ type: 'chat.item', chatId: chatId!, index, item }));
		await expect(panel.getByText(`Message ${index}`, { exact: true })).toBeAttached();
	}

	await append(1);
	await expect.poll(distance).toBeLessThan(2);
	for (let lines = 21; lines <= 25; lines++) {
		route!.send(JSON.stringify({ type: 'chat.item', chatId: chatId!, index: 1, item: { kind: 'text', text: 'Message 1\n\n' + 'New streaming line.\n\n'.repeat(lines), streaming: true, phase: 'commentary' } }));
		await expect(panel.locator('.transcript p')).toHaveCount(82 + lines);
		await expect.poll(distance).toBeLessThan(2);
	}

	await scroll(false);
	await expect.poll(distance).toBeGreaterThan(250);
	const top = await transcript.evaluate((node) => node.scrollTop);
	await append(2);
	await expect.poll(() => transcript.evaluate((node) => node.scrollTop)).toBe(top);
	await expect.poll(distance).toBeGreaterThan(500);

	await scroll(true);
	await expect.poll(distance).toBeLessThan(2);
	await append(3);
	await expect.poll(distance).toBeLessThan(2);

	async function scroll(bottom: boolean) {
		await transcript.evaluate((node, bottom) => new Promise<void>((resolve) => {
			node.addEventListener('scroll', () => resolve(), { once: true });
			node.scrollTop = bottom ? node.scrollHeight : node.scrollTop - 300;
		}), bottom);
	}

	await closeProject(page, name);
});
