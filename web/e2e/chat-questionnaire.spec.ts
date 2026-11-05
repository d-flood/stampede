import { expect, test } from './fixtures.ts';
import type { Locator, Page, WebSocketRoute } from '@playwright/test';
import { copyFileSync, mkdirSync, readFileSync, unlinkSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { closeProject, newTab, openProject } from './shell.ts';

const NOTE = 'Add a note or your own answer';

async function launch(page: Page, agent: string, name: string) {
	const path = `${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`;
	mkdirSync(path, { recursive: true });
	copyFileSync(fileURLToPath(new URL('../static/icons/icon-512.png', import.meta.url)), `${path}/preview.png`);
	await openProject(page, name);
	await newTab(page, `Rich ${agent} Questions`);
	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	await panel.getByRole('textbox', { name: 'Composer' }).fill('Ask me questions.');
	await panel.getByRole('button', { name: 'Send' }).click();
	const card = panel.getByRole('region', { name: 'Question', exact: true });
	await expect(card).toContainText('6 questions');
	unlinkSync(`${path}/preview.png`);
	return { panel, card, opened: card.locator('li.open'), send: card.locator('[data-send]'), path };
}

const row = (card: Locator, title: string) => card.getByRole('button', { name: new RegExp(`^${title}`) });

async function fits(card: Locator) {
	await expect.poll(() => card.evaluate((node) => node.scrollWidth <= node.clientWidth && node.getBoundingClientRect().bottom <= innerHeight)).toBe(true);
}

function reply(path: string) {
	return expect.poll(() => {
		try { return JSON.parse(readFileSync(`${path}/questions-result.json`, 'utf8')).structuredContent; }
		catch { return null; }
	});
}

for (const agent of ['claude-code', 'codex']) {
	test(`${agent} rich questions collapse answers into rows and keep durable history`, { tag: agent === 'codex' ? '@desktop' : [] }, async ({ page }, { project }) => {
		const name = `rich-${agent}-${project.name}`;
		const { panel, card, opened, send, path } = await launch(page, agent, name);
		await expect(panel.getByText('ask_questions')).toHaveCount(0);
		await expect(card.locator('strong').filter({ hasText: 'storage' })).toHaveText('storage');
		await expect(card.locator('pre code')).toHaveText('const local = true;\n');
		await expect(card.getByText('Recommended', { exact: true })).toBeVisible();
		await expect(card.locator('.preview')).toHaveText('local = true');
		const image = card.getByRole('img', { name: 'Storage preview' });
		await expect.poll(() => image.evaluate((node: HTMLImageElement) => node.complete && node.naturalWidth > 0)).toBe(true);
		await expect(send).toBeDisabled();
		await expect(card.getByRole('radio', { name: 'Local, recommended', exact: true })).toHaveAttribute('type', 'radio');

		await card.getByText('local-first').click();
		await expect(row(card, 'Storage')).toContainText('Local');
		await expect(row(card, 'Tools')).toHaveAttribute('aria-expanded', 'true');
		await fits(card);

		await opened.getByRole('checkbox', { name: 'Read', exact: true }).click();
		await opened.getByRole('checkbox', { name: 'Write', exact: true }).click();
		await expect(opened.getByRole('checkbox', { name: 'Read', exact: true })).toBeChecked();
		await expect(opened.getByRole('checkbox', { name: 'Write', exact: true })).toBeChecked();
		await opened.getByRole('textbox', { name: NOTE }).fill('Ask before deleting.');
		await card.getByRole('button', { name: 'Next question', exact: true }).click();
		await opened.getByRole('textbox', { name: 'Your answer', exact: true }).fill('First line\nSecond line');
		await card.getByRole('button', { name: 'Next question', exact: true }).click();
		await opened.getByRole('textbox', { name: NOTE }).fill('A custom alternative');
		await card.getByRole('button', { name: 'Next question', exact: true }).click();
		await expect(row(card, 'Optional')).toHaveAttribute('aria-expanded', 'true');
		await card.getByRole('button', { name: 'Next question', exact: true }).click();
		await expect(row(card, 'Finish')).toHaveAttribute('aria-expanded', 'true');
		await opened.getByRole('textbox', { name: 'Your answer', exact: true }).fill('Done');
		await expect(send).toHaveText('Send 5, skip 1');
		await expect(row(card, 'Optional')).toContainText('Not answered');

		await row(card, 'Storage').click();
		await opened.getByRole('textbox', { name: NOTE }).fill('Keep offline support.');
		await opened.getByRole('radio', { name: 'Remote', exact: true }).click();
		await expect(row(card, 'Storage')).toContainText('Remote. Keep offline support.');
		await expect(row(card, 'Optional')).toHaveAttribute('aria-expanded', 'true');
		await fits(card);
		await send.click();
		await expect(card).toHaveCount(0);

		await reply(path).toEqual({ answers: {
			storage: { selected: ['remote'], text: 'Keep offline support.', skipped: false },
			tools: { selected: ['read', 'write'], text: 'Ask before deleting.', skipped: false },
			notes: { selected: [], text: 'First line\nSecond line', skipped: false },
			alternative: { selected: [], text: 'A custom alternative', skipped: false },
			optional: { selected: [], text: '', skipped: true },
			finish: { selected: [], text: 'Done', skipped: false }
		}, cancelled: false });

		const history = panel.getByRole('group', { name: 'Answered questions', exact: true });
		await expect(history).toHaveCount(1);
		await expect(panel.getByText('ask_questions')).toHaveCount(0);
		await expect(history.locator('summary')).toContainText('5 of 6 answered');
		await expect(history.locator('summary')).toContainText('Storage: Remote. Keep offline support.');
		await expect(history.locator('summary')).toContainText('Optional: skipped');
		await history.locator('summary').click();
		await expect(history.locator('.option.selected').first()).toContainText('Remote');
		await expect(history.getByText('Keep offline support.', { exact: true })).toBeVisible();
		await expect(history.getByText('First line\nSecond line', { exact: true })).toBeVisible();
		await expect(history.getByText('Skipped', { exact: true })).toBeVisible();
		await expect.poll(() => history.evaluate((node) => node.scrollWidth <= node.clientWidth)).toBe(true);
		await page.reload();
		const restored = page.getByRole('group', { name: 'Answered questions', exact: true });
		await restored.locator('summary').click();
		await expect(restored.getByText('Keep offline support.', { exact: true })).toBeVisible();
		await expect.poll(() => restored.getByRole('img', { name: 'Storage preview' }).evaluate((node: HTMLImageElement) => node.naturalWidth)).toBeGreaterThan(0);
		await closeProject(page, name);
	});

	test(`${agent} rich questions can be dismissed and leave a record`, { tag: '@desktop' }, async ({ page }, { project }) => {
		const name = `rich-cancel-${agent}-${project.name}`;
		const { panel, card, send, path } = await launch(page, agent, name);
		await expect(send).toBeDisabled();
		await card.getByRole('button', { name: 'Dismiss', exact: true }).click();
		await expect(card).toHaveCount(0);
		await reply(path).toEqual({ answers: {}, cancelled: true });
		await expect(panel.getByRole('group', { name: 'Unanswered questions', exact: true }).locator('summary')).toContainText('6 questions dismissed');
		await closeProject(page, name);
	});
}

test('dismissing asks first once there are answers to lose', { tag: '@desktop' }, async ({ page }, { project }) => {
	const name = `rich-discard-${project.name}`;
	const { card, opened, path } = await launch(page, 'claude-code', name);
	await opened.getByRole('radio', { name: 'Local, recommended', exact: true }).click();
	await expect(row(card, 'Storage')).toContainText('Local');
	await card.getByRole('button', { name: 'Dismiss', exact: true }).click();
	await expect(card).toBeVisible();
	await card.getByRole('button', { name: 'Discard 1 answer?', exact: true }).click();
	await expect(card).toHaveCount(0);
	await reply(path).toEqual({ answers: {}, cancelled: true });
	await closeProject(page, name);
});

test('answers survive a reconnect and a reload, then clear once sent', { tag: '@desktop' }, async ({ page }, { project }) => {
	let route: WebSocketRoute | undefined;
	await page.routeWebSocket('**/ws', (socket) => {
		route = socket;
		socket.connectToServer();
	});
	const name = `rich-draft-${project.name}`;
	const { card, opened, send } = await launch(page, 'claude-code', name);
	await opened.getByRole('radio', { name: 'Local, recommended', exact: true }).click();
	await expect(row(card, 'Tools')).toHaveAttribute('aria-expanded', 'true');
	await opened.getByRole('textbox', { name: NOTE }).fill('Ask before deleting.');
	const reconnecting = page.getByRole('status').filter({ hasText: 'Reconnecting…' });
	await route!.close();
	await expect(reconnecting).toBeVisible();
	await expect(reconnecting).toBeHidden({ timeout: 10_000 });
	await expect(row(card, 'Storage')).toContainText('Local');
	await expect(row(card, 'Tools')).toContainText('Ask before deleting.');
	await page.reload();
	await expect(row(card, 'Storage')).toContainText('Local');
	await expect(row(card, 'Tools')).toContainText('Ask before deleting.');
	await expect(row(card, 'Notes')).toHaveAttribute('aria-expanded', 'true');
	await expect(send).toHaveText('Send 2, skip 4');
	await send.click();
	await expect(card).toHaveCount(0);
	await expect.poll(() => page.evaluate(() => Object.keys(localStorage).filter((key) => key.startsWith('question:')))).toEqual([]);
	await closeProject(page, name);
});

test('typing an instruction keeps the answers given so far', { tag: '@desktop' }, async ({ page }, { project }) => {
	const name = `rich-redirect-${project.name}`;
	const { panel, card, opened, path } = await launch(page, 'claude-code', name);
	await opened.getByRole('radio', { name: 'Local, recommended', exact: true }).click();
	await expect(row(card, 'Storage')).toContainText('Local');
	await panel.getByRole('textbox', { name: 'Composer' }).fill('Use the defaults for the rest.');
	await panel.getByRole('button', { name: 'Send', exact: true }).click();
	await expect(card).toHaveCount(0);
	await reply(path).toEqual({ answers: { storage: { selected: ['local'], text: '', skipped: false } }, cancelled: true, instruction: 'Use the defaults for the rest.' });
	const history = panel.getByRole('group', { name: 'Answered questions', exact: true });
	await expect(history.locator('summary')).toContainText('1 of 6 answered');
	await expect(panel.locator('.instruction')).toHaveText('Use the defaults for the rest.');
	await page.reload();
	await expect(page.locator('.instruction')).toHaveText('Use the defaults for the rest.');
	await closeProject(page, name);
});

test('keys pick options and move on, but arrows only change the selection', { tag: '@desktop' }, async ({ page }, { project }) => {
	const name = `rich-keys-${project.name}`;
	const { panel, card, opened } = await launch(page, 'claude-code', name);
	const hint = opened.locator('.key').first();
	await expect.poll(() => hint.evaluate((node) => getComputedStyle(node).opacity)).toBe('0');
	await opened.getByRole('radio', { name: 'Remote', exact: true }).focus();
	await expect.poll(() => hint.evaluate((node) => getComputedStyle(node).opacity)).toBe('1');

	await page.keyboard.press('1');
	await expect(row(card, 'Storage')).toContainText('Local');
	await expect(row(card, 'Tools')).toHaveAttribute('aria-expanded', 'true');
	await expect(opened.getByRole('checkbox', { name: 'Read', exact: true })).toBeFocused();

	await page.keyboard.press('2');
	await expect(opened.getByRole('checkbox', { name: 'Write', exact: true })).toBeChecked();
	await expect(row(card, 'Tools')).toHaveAttribute('aria-expanded', 'true');
	await page.keyboard.press('Enter');
	await expect(row(card, 'Notes')).toHaveAttribute('aria-expanded', 'true');
	await expect(opened.getByRole('textbox', { name: 'Your answer', exact: true })).toBeFocused();
	await page.keyboard.type('Typed 1 line');
	await page.keyboard.press('Shift+Enter');
	await expect(row(card, 'Notes')).toHaveAttribute('aria-expanded', 'true');
	await expect(opened.getByRole('textbox', { name: 'Your answer', exact: true })).toHaveValue('Typed 1 line\n');
	await page.keyboard.press('Enter');
	await expect(row(card, 'Alternative')).toHaveAttribute('aria-expanded', 'true');

	await row(card, 'Storage').click();
	await expect(opened.getByRole('radio', { name: 'Local, recommended', exact: true })).toBeFocused();
	await page.keyboard.press('ArrowDown');
	await expect(opened.getByRole('radio', { name: 'Remote', exact: true })).toBeChecked();
	await page.waitForTimeout(400);
	await expect(row(card, 'Storage')).toHaveAttribute('aria-expanded', 'true');
	await panel.getByRole('button', { name: 'Hide questions', exact: true }).click();
	await expect(opened).toHaveCount(0);
	await closeProject(page, name);
});

test('questions slide open and closed, and do not animate under reduced motion', { tag: '@desktop' }, async ({ page }, { project }) => {
	await page.emulateMedia({ reducedMotion: 'no-preference' });
	const name = `rich-motion-${project.name}`;
	const { card, opened } = await launch(page, 'claude-code', name);
	const animating = () => card.evaluate(async (node) => {
		(node.querySelector('li:not(.open) .row') as HTMLElement).click();
		await new Promise(requestAnimationFrame);
		return Array.from(node.querySelectorAll('.panel')).filter((panel) => panel.getAnimations().length).length;
	});
	expect(await animating()).toBe(2);
	await expect.poll(() => card.locator('.panel').count()).toBe(1);
	await expect.poll(() => opened.locator('.panel').evaluate((node) => node.getAnimations().length)).toBe(0);
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await expect.poll(() => animating()).toBe(0);
	await closeProject(page, name);
});
