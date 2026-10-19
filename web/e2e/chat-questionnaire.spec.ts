import { expect, test } from './fixtures.ts';
import type { Locator, Page } from '@playwright/test';
import { copyFileSync, mkdirSync, readFileSync, unlinkSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { closeProject, openProject } from './shell.ts';

async function launch(page: Page, agent: string, name: string) {
	const path = `${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`;
	mkdirSync(path, { recursive: true });
	copyFileSync(fileURLToPath(new URL('../static/icons/icon-512.png', import.meta.url)), `${path}/preview.png`);
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: `Rich ${agent} Questions`, exact: true }).click();
	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	await panel.getByRole('textbox', { name: 'Composer' }).fill('Ask me questions.');
	await panel.getByRole('button', { name: 'Send' }).click();
	const card = panel.getByRole('region', { name: 'Question', exact: true });
	await expect(card).toContainText('Question 1 of 6');
	unlinkSync(`${path}/preview.png`);
	return { panel, card, path };
}

async function fits(card: Locator) {
	await expect.poll(() => card.evaluate((node) => node.scrollWidth <= node.clientWidth && node.getBoundingClientRect().bottom <= innerHeight)).toBe(true);
}

for (const agent of ['claude-code', 'codex']) {
	test(`${agent} rich questions support choices, text, editing, review and durable history`, async ({ page }, info) => {
		const { project } = info;
		const name = `rich-${agent}-${project.name}`;
		const { panel, card, path } = await launch(page, agent, name);
		await expect(card.locator('strong').filter({ hasText: 'storage' })).toHaveText('storage');
		await expect(card.locator('pre code')).toHaveText('const local = true;\n');
		await expect(card.getByText('Recommended', { exact: true })).toBeVisible();
		await expect(card.locator('.preview')).toHaveText('local = true');
		const image = card.getByRole('img', { name: 'Storage preview' });
		await expect.poll(() => image.evaluate((node: HTMLImageElement) => node.complete && node.naturalWidth > 0)).toBe(true);
		await expect(card.getByRole('navigation', { name: 'Questions' })).toHaveCount(project.name === 'pixel' ? 0 : 1);
		await expect(card.getByRole('button', { name: 'Next', exact: true })).toBeDisabled();
		await card.getByRole('button', { name: 'Local Recommended', exact: true }).click();
		await card.getByRole('textbox', { name: 'Additional text (optional)' }).fill('Keep offline support.');
		await fits(card);
		await card.screenshot({ path: info.outputPath('questions.png') });
		await card.getByRole('button', { name: 'Next', exact: true }).click();
		await expect(card).toContainText('Question 2 of 6');
		await card.getByRole('button', { name: 'Read', exact: true }).click();
		await card.getByRole('button', { name: 'Write', exact: true }).click();
		await expect(card.getByRole('button', { name: 'Read', exact: true })).toHaveAttribute('aria-pressed', 'true');
		await expect(card.getByRole('button', { name: 'Write', exact: true })).toHaveAttribute('aria-pressed', 'true');
		await card.getByRole('textbox', { name: 'Additional text (optional)' }).fill('Ask before deleting.');
		await card.getByRole('button', { name: 'Back', exact: true }).click();
		await expect(card.getByRole('textbox', { name: 'Additional text (optional)' })).toHaveValue('Keep offline support.');
		await expect(card.getByRole('button', { name: 'Local Recommended', exact: true })).toHaveAttribute('aria-pressed', 'true');
		await card.getByRole('button', { name: 'Next', exact: true }).click();
		await expect(card.getByRole('textbox', { name: 'Additional text (optional)' })).toHaveValue('Ask before deleting.');
		await card.getByRole('button', { name: 'Next', exact: true }).click();
		await card.getByRole('textbox', { name: 'Answer', exact: true }).fill('First line\nSecond line');
		await card.getByRole('button', { name: 'Next', exact: true }).click();
		await card.getByRole('textbox', { name: 'Additional text (optional)' }).fill('A custom alternative');
		await card.getByRole('button', { name: 'Next', exact: true }).click();
		await card.getByRole('button', { name: 'Skip', exact: true }).click();
		await expect(card).toContainText('Question 6 of 6');
		await card.getByRole('textbox', { name: 'Answer', exact: true }).fill('Done');
		await card.getByRole('button', { name: 'Review answers', exact: true }).click();
		const review = card.getByRole('list', { name: 'Answer review' });
		await expect(review.getByText('First line\nSecond line', { exact: true })).toBeVisible();
		await expect(review.getByText('Skipped', { exact: true })).toBeVisible();
		await expect(panel.getByRole('group', { name: 'Answered questions', exact: true })).toHaveCount(0);
		await card.getByRole('button', { name: 'Edit Storage', exact: true }).click();
		await card.getByRole('button', { name: 'Remote', exact: true }).click();
		await expect(card.getByRole('button', { name: 'Local Recommended', exact: true })).toHaveAttribute('aria-pressed', 'false');
		await expect(card.getByRole('button', { name: 'Remote', exact: true })).toHaveAttribute('aria-pressed', 'true');
		await card.getByRole('button', { name: 'Review', exact: true }).click();
		await fits(card);
		await card.getByRole('button', { name: 'Submit answers', exact: true }).click();
		await expect(card).toHaveCount(0);
		const expected = {
			storage: { selected: ['remote'], text: 'Keep offline support.', skipped: false },
			tools: { selected: ['read', 'write'], text: 'Ask before deleting.', skipped: false },
			notes: { selected: [], text: 'First line\nSecond line', skipped: false },
			alternative: { selected: [], text: 'A custom alternative', skipped: false },
			optional: { selected: [], text: '', skipped: true },
			finish: { selected: [], text: 'Done', skipped: false }
		};
		await expect.poll(() => {
			try { return JSON.parse(readFileSync(`${path}/questions-result.json`, 'utf8')).structuredContent; }
			catch { return null; }
		}).toEqual({ answers: expected, cancelled: false });
		const history = panel.getByRole('group', { name: 'Answered questions', exact: true });
		await expect(history).toHaveCount(1);
		await history.locator('summary').click();
		await expect(history.getByText('Answer: Remote\nKeep offline support.', { exact: true })).toBeVisible();
		await expect(history.getByText('Answer: First line\nSecond line', { exact: true })).toBeVisible();
		await expect(history.getByText('Answer: Skipped', { exact: true })).toBeVisible();
		await expect.poll(() => history.evaluate((node) => node.scrollWidth <= node.clientWidth)).toBe(true);
		await page.reload();
		const restored = page.getByRole('group', { name: 'Answered questions', exact: true });
		await restored.locator('summary').click();
		await expect(restored.getByText('Answer: Remote\nKeep offline support.', { exact: true })).toBeVisible();
		await expect.poll(() => restored.getByRole('img', { name: 'Storage preview' }).evaluate((node: HTMLImageElement) => node.naturalWidth)).toBeGreaterThan(0);
		await closeProject(page, name);
	});

	test(`${agent} rich questions can be cancelled without submitting`, async ({ page }, { project }) => {
		const name = `rich-cancel-${agent}-${project.name}`;
		const { panel, card, path } = await launch(page, agent, name);
		await card.getByRole('button', { name: 'Review', exact: true }).click();
		await expect(card.getByRole('button', { name: 'Submit answers', exact: true })).toBeDisabled();
		await card.getByRole('button', { name: 'Cancel questions', exact: true }).click();
		await expect(card).toHaveCount(0);
		await expect.poll(() => {
			try { return JSON.parse(readFileSync(`${path}/questions-result.json`, 'utf8')).structuredContent; }
			catch { return null; }
		}).toEqual({ answers: {}, cancelled: true });
		await expect(panel.getByRole('group', { name: 'Answered questions', exact: true })).toHaveCount(0);
		await closeProject(page, name);
	});
}
