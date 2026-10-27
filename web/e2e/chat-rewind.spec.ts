import type { ClientMessage, Item, ServerMessage } from '#lib/device.svelte.ts';
import type { WebSocketRoute } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { expect, test } from './fixtures.ts';
import { closeProject, openProject } from './shell.ts';

for (const agent of ['claude-code', 'codex'] as const) {
	test(`${agent} previews the exact rewind boundary and steps through user messages`, { tag: agent === 'codex' ? '@desktop' : [] }, async ({ page }, { project }) => {
		let route: WebSocketRoute;
		let chatId: string;
		const sent: ClientMessage[] = [];
		const items: Item[] = Array.from({ length: 6 }, (_, index): Item[] => [
			{ kind: 'user', text: `Prompt ${index + 1}`, images: 0, ...(agent === 'codex' && index === 2 ? { rewindFrom: 0 } : {}) },
			{ kind: 'text', text: `Reply ${index + 1}\n\n` + 'A paragraph of conversation.\n\n'.repeat(30), streaming: false, phase: 'final' }
		]).flat();
		await page.routeWebSocket('**/ws', (socket) => {
			const server = socket.connectToServer();
			socket.onMessage((data) => {
				if (typeof data === 'string') {
					const message: ClientMessage = JSON.parse(data);
					sent.push(message);
					if (message.type === 'chat.run' && message.command === 'rewind') return;
				}
				server.send(data);
			});
			server.onMessage((data) => {
				if (typeof data !== 'string') return socket.send(data);
				const message: ServerMessage = JSON.parse(data);
				if (message.type === 'chat.created') { route = socket; chatId = message.chat.id; message.chat.agent = agent; }
				if (message.type === 'chats.changed') for (const chat of message.chats) if (chat.id === chatId) chat.agent = agent;
				if (message.type === 'chat.capabilities' && message.chatId === chatId) message.capabilities.rewindsFiles = agent === 'claude-code';
				if (message.type === 'chat.items' && message.chatId === chatId) message.items = items;
				socket.send(JSON.stringify(message));
			});
		});
		const name = `rewind-review-${agent}-${project.name}`;
		mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`);
		await openProject(page, name);
		await page.getByRole('button', { name: 'New Tab' }).click();
		await page.getByRole('menuitem', { name: 'Fake Claude', exact: true }).click();
		const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
		const nav = panel.getByRole('navigation', { name: 'User messages', exact: true });
		const transcript = panel.locator('.transcript');
		const column = transcript.locator('.column');
		const before = await column.evaluate((node) => ({ left: node.getBoundingClientRect().left, width: node.getBoundingClientRect().width }));
		const opacity = () => nav.evaluate((node) => getComputedStyle(node).opacity);
		const message = (number: number) => panel.getByRole('region', { name: `User message ${number}`, exact: true });
		const landed = (number: number) => expect.poll(() => message(number).evaluate((node) => Math.abs(node.getBoundingClientRect().top - node.closest('.conversation')!.querySelector('nav')!.getBoundingClientRect().bottom))).toBeLessThan(2);
		await expect.poll(opacity).toBe('0');
		await expect.poll(() => transcript.evaluate((node) => node.getBoundingClientRect().width === node.parentElement!.getBoundingClientRect().width)).toBe(true);
		await transcript.evaluate((node) => { node.scrollTop -= 250; });
		await expect.poll(opacity).toBe('1');
		await expect(nav.getByRole('button', { name: 'Message 6 of 6: Prompt 6', exact: true })).toBeVisible();
		await expect.poll(() => column.evaluate((node) => ({ left: node.getBoundingClientRect().left, width: node.getBoundingClientRect().width }))).toEqual(before);
		await transcript.evaluate((node) => { node.scrollTop += 100; });
		await expect.poll(opacity).toBe('0');
		await transcript.evaluate((node) => { node.scrollTop -= 100; });
		await expect.poll(opacity).toBe('1');
		const previous = nav.getByRole('button', { name: 'Previous message', exact: true });
		const next = nav.getByRole('button', { name: 'Next message', exact: true });
		for (const button of [previous, next]) {
			await expect.poll(() => button.evaluate((node) => {
				const rect = node.getBoundingClientRect();
				const minimum = parseFloat(getComputedStyle(node).getPropertyValue('--target-min'));
				return rect.width >= minimum && rect.height >= minimum;
			})).toBe(true);
		}
		await previous.click();
		await landed(6);
		await previous.click();
		await landed(5);
		await expect(nav.getByRole('button', { name: 'Message 5 of 6: Prompt 5', exact: true })).toBeVisible();
		await next.click();
		await landed(6);
		await expect.poll(opacity).toBe('1');
		await nav.getByRole('button', { name: 'Message 6 of 6: Prompt 6', exact: true }).click();
		await page.getByRole('menuitemcheckbox', { name: '1. Prompt 1', exact: true }).click();
		await landed(1);
		await transcript.evaluate((node) => { node.scrollTop = 0; });
		await landed(1);
		await nav.getByRole('button', { name: 'Message 1 of 6: Prompt 1', exact: true }).click();
		await page.getByRole('menuitemcheckbox', { name: '3. Prompt 3', exact: true }).click();
		await landed(3);
		await expect(nav.getByRole('button', { name: 'Message 3 of 6: Prompt 3', exact: true })).toBeVisible();
		await expect.poll(() => nav.evaluate((node) => {
			const rect = node.getBoundingClientRect();
			const panel = node.closest('.panel')!.getBoundingClientRect();
			return rect.right <= panel.right && rect.top >= panel.top && rect.bottom <= panel.bottom;
		})).toBe(true);
		await message(3).getByRole('button', { name: 'Rewind from message 3', exact: true }).click();
		const boundary = agent === 'codex' ? 1 : 3;
		const review = page.getByRole('dialog', { name: `Rewind before message ${boundary}?` });
		await expect(review).toContainText('Message 3 will be removed, including its prompt and attachments.');
		await expect(review).toContainText(`Message ${boundary} and every later message, agent reply, and tool result.`);
		await expect(review).toContainText(agent === 'claude-code' ? 'Restore checkpointed file edits to their state before message 3.' : 'Files are left unchanged, including edits made by the removed turns.');
		if (agent === 'codex') await expect(review).toContainText('That entire turn will be removed too.');
		await expect.poll(() => review.evaluate((node) => {
			const rect = node.getBoundingClientRect();
			return rect.left >= 0 && rect.right <= innerWidth && rect.top >= 0 && rect.bottom <= innerHeight;
		})).toBe(true);
		expect(sent.filter((message) => message.type === 'chat.run')).toHaveLength(0);
		await review.getByRole('button', { name: 'Cancel', exact: true }).click();
		await expect(review).toHaveCount(0);
		await message(3).getByRole('button', { name: 'Rewind from message 3', exact: true }).click();
		const confirm = review.getByRole('button', { name: `Rewind before message ${boundary}`, exact: true });
		route!.send(JSON.stringify({ type: 'chat.status', chatId: chatId!, status: 'working' }));
		await expect(confirm).toBeDisabled();
		await expect(review.getByRole('status')).toContainText('Wait for the current turn');
		route!.send(JSON.stringify({ type: 'chat.status', chatId: chatId!, status: 'done' }));
		route!.send(JSON.stringify({ type: 'chat.queued', chatId: chatId!, queued: ['Later'], revision: 1 }));
		await expect(confirm).toBeDisabled();
		await expect(review.getByRole('status')).toHaveText('Cancel queued messages before rewinding.');
		route!.send(JSON.stringify({ type: 'chat.queued', chatId: chatId!, queued: [], revision: 2 }));
		await expect(confirm).toBeEnabled();
		await confirm.click();
		await expect(review).toHaveCount(0);
		await expect.poll(() => sent.filter((message) => message.type === 'chat.run')).toEqual([{ type: 'chat.run', chatId: chatId!, command: 'rewind', value: 4 }]);
		await expect.poll(() => panel.evaluate((node) => node.scrollWidth <= node.clientWidth)).toBe(true);
		await closeProject(page, name);
	});
}
