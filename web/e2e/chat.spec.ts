import type { ServerMessage } from '#lib/device.svelte.ts';
import { expect, test } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { openProject } from './shell.ts';

test('a Claude Code Chat streams its reply, shows done, and asks before closing while working', async ({ page }, { project }) => {
	const name = `chat-${project.name}`;
	mkdirSync(`${process.env.OIKONOMOS_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude as Chat' }).click();
	const tab = page.getByRole('tab', { name: /^Fake Claude/ });
	await expect(tab).toHaveAttribute('aria-selected', 'true');
	await expect(tab.getByTitle('done')).toBeVisible();

	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	const composer = panel.getByRole('textbox', { name: 'Composer' });
	await composer.fill('Greet the reader');
	await panel.getByRole('button', { name: 'Send' }).click();
	await expect(panel.getByText('Greet the reader')).toBeVisible();
	await expect(panel.locator('strong', { hasText: 'welcome' })).toBeVisible();
	await expect(panel.getByText(/^Hello and welcome!/)).toBeVisible();
	await expect(tab.getByTitle('done')).toBeVisible();

	await composer.fill('Anything else?');
	await panel.getByRole('button', { name: 'Send' }).click();
	await expect(tab.getByTitle('working')).toBeVisible();
	await tab.click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'Close', exact: true }).click();
	const confirm = page.getByRole('dialog', { name: 'Close Fake Claude?' });
	await expect(confirm).toBeVisible();
	await confirm.getByRole('button', { name: 'Close' }).click();
	await expect(tab).toHaveCount(0);
});

test('a Codex Chat streams its reply, shows done, and closes', async ({ page }, { project }) => {
	const name = `codex-chat-${project.name}`;
	mkdirSync(`${process.env.OIKONOMOS_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Fake Codex as Chat' }).click();
	const tab = page.getByRole('tab', { name: /^Fake Codex/ });
	await expect(tab).toHaveAttribute('aria-selected', 'true');
	await expect(tab.getByTitle('done')).toBeVisible();

	const prompt = 'Greet the reader in one short sentence with one word in **bold**. Use no names and no tools.';
	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	await panel.getByRole('textbox', { name: 'Composer' }).fill(prompt);
	await panel.getByRole('button', { name: 'Send' }).click();
	await expect(panel.getByText(prompt)).toBeVisible();
	await expect(panel.locator('strong', { hasText: 'glad' })).toBeVisible();
	await expect(panel.getByText('Welcome—glad you’re here.')).toBeVisible();
	await expect(tab.getByTitle('done')).toBeVisible();

	await tab.click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'Close', exact: true }).click();
	await expect(tab).toHaveCount(0);
});

test('a Chat that finishes unseen needs attention and opens from the Attention list or a notification', async ({ page }, { project }) => {
	const name = `attention-${project.name}`;
	mkdirSync(`${process.env.OIKONOMOS_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Terminal' }).click();
	const terminalTab = page.getByRole('tablist', { name: 'Tabs' }).getByRole('tab').first();
	await expect(terminalTab).toHaveAttribute('aria-selected', 'true');

	const { projectId, chatId } = await page.evaluate(finishElsewhere, name);
	const chatTab = page.locator(`[data-tab="${chatId}"]`);
	await expect(chatTab).toHaveAttribute('aria-selected', 'false');
	const mark = page.locator(`nav[aria-label="Projects"] button[aria-label="${name}"]`).getByText('1', { exact: true });
	await expect(mark).toBeAttached();
	const list = page.getByRole('button', { name: 'Attention list' });
	const entry =
		project.name === 'pixel'
			? page.getByRole('dialog', { name: 'Needs attention' }).getByRole('button', { name: `Fake Claude in ${name}` })
			: page.getByRole('menuitem', { name: `Fake Claude in ${name}` });
	await list.click();
	await entry.click();
	await expect(chatTab).toHaveAttribute('aria-selected', 'true');
	await list.click();
	await expect(entry).toHaveCount(0);
	await expect(mark).toHaveCount(0);
	await page.keyboard.press('Escape');

	// An unknown Tab falls back to the last one, so another Chat keeps this one from being chosen by default.
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude as Chat' }).click();
	await expect(page.getByRole('tablist', { name: 'Tabs' }).getByRole('tab').last()).toHaveAttribute('aria-selected', 'true');
	await expect(chatTab).toHaveAttribute('aria-selected', 'false');
	const notification = { projectId, chatId, label: 'Fake Claude', status: 'done' };
	await page.evaluate((data) => navigator.serviceWorker.dispatchEvent(new MessageEvent('message', { data })), notification);
	await expect(chatTab).toHaveAttribute('aria-selected', 'true');

	const saved = page.waitForResponse((response) => {
		const request = response.request();
		const layout = request.method() === 'PUT' && JSON.parse(request.postData() ?? '{}');
		return layout?.projects?.[projectId]?.active?.kind === 'terminal';
	});
	await terminalTab.click();
	await saved;
	await page.goto(`/?${new URLSearchParams({ project: projectId, tab: chatId })}`);
	await expect(chatTab).toHaveAttribute('aria-selected', 'true');
});

/** Runs a Chat's turn to done from a second Device that shows nothing. */
async function finishElsewhere(name: string) {
	type Of<T> = Extract<ServerMessage, { type: T }>;
	const socket = new WebSocket(new URL('/ws', location.href.replace(/^http/, 'ws')));
	const next = <T extends ServerMessage['type']>(type: T, matches: (message: Of<T>) => boolean = () => true) =>
		new Promise<Of<T>>((resolve) => {
			socket.addEventListener('message', function listener({ data }) {
				const message = JSON.parse(data);
				if (message.type !== type || !matches(message)) return;
				socket.removeEventListener('message', listener);
				resolve(message);
			});
		});
	await new Promise((resolve) => (socket.onopen = resolve));
	socket.send(JSON.stringify({ type: 'hello', deviceId: crypto.randomUUID() }));
	socket.send(JSON.stringify({ type: 'projects.list' }));
	const { projects } = await next('projects.changed');
	const projectId = projects.find((project) => project.path.endsWith(`/${name}`))!.id;
	socket.send(JSON.stringify({ type: 'chat.create', projectId, rig: 'Fake Claude' }));
	const { chat } = await next('chat.created');
	socket.send(JSON.stringify({ type: 'chat.send', chatId: chat.id, text: 'Greet the reader' }));
	await next('chat.status', (message) => message.chatId === chat.id && message.status === 'done');
	socket.close();
	return { projectId, chatId: chat.id };
}
