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

test('the initial message stays below the navigator at the top of a chat', async ({ page }) => {
	let chatId: string;
	const items: Item[] = [
		{ kind: 'user', text: "On mobile, I'd like the swiping animation to be faster between e.g. projects and tabs. Also, can the order of screens follow the sidebar side setting?", images: 0 },
		{ kind: 'text', text: 'A paragraph of conversation.\n\n'.repeat(50), streaming: false, phase: 'final' }
	];
	await page.routeWebSocket('**/ws', (socket) => {
		const server = socket.connectToServer();
		server.onMessage((data) => {
			if (typeof data !== 'string') return socket.send(data);
			const message: ServerMessage = JSON.parse(data);
			if (message.type === 'chat.created') chatId = message.chat.id;
			socket.send(message.type === 'chat.items' && message.chatId === chatId ? JSON.stringify({ ...message, items }) : data);
		});
	});
	await openProject(page, 'alpha');
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude', exact: true }).click();
	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	const transcript = panel.locator('.transcript');
	const nav = panel.getByRole('navigation', { name: 'User messages', exact: true });
	const message = panel.getByRole('region', { name: 'User message 1', exact: true });
	await expect(message).toBeAttached();
	await expect.poll(() => transcript.evaluate((node) => node.scrollHeight - node.scrollTop - node.clientHeight)).toBeLessThan(2);
	await transcript.evaluate((node) => { node.scrollTop = 0; });
	await expect(nav).toHaveCSS('opacity', '1');
	const unobscured = () => expect.poll(async () => {
		const [prompt, bar] = await Promise.all([message.boundingBox(), nav.boundingBox()]);
		return prompt!.y - (bar!.y + bar!.height);
	}).toBeGreaterThanOrEqual(0);
	await unobscured();
	await expect(nav.getByRole('button', { name: 'Previous message', exact: true })).toBeDisabled();
	await page.getByRole('button', { name: 'Settings', exact: true }).click();
	const settings = page.getByRole('dialog', { name: 'Settings', exact: true });
	await settings.getByRole('combobox', { name: 'Text size', exact: true }).selectOption('22');
	await settings.getByRole('button', { name: 'Done', exact: true }).click();
	await expect(nav).toHaveCSS('opacity', '1');
	await unobscured();
});
