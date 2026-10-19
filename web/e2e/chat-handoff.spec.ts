import type { ClientMessage } from '#lib/device.svelte.ts';
import { appendFileSync, existsSync, mkdirSync, unlinkSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test } from './fixtures.ts';
import { openProject } from './shell.ts';

const summary = 'Continue implementing authentication.\n\nKeep local-first behavior. Login is unfinished; next inspect src/auth.ts. Verification: no tests run.';

for (const agent of ['claude-code', 'codex'] as const) {
	test(`${agent} prepares an editable handoff in the chosen Rig`, async ({ page, request }) => {
		const root = process.env.STAMPEDE_E2E_ROOT!;
		const fixture = fileURLToPath(new URL(`../../crates/stampede/tests/fixtures/chats/${agent}/handoff.synthetic.jsonl`, import.meta.url));
		mkdirSync(`${root}/handoff`);
		appendFileSync(`${root}/config.toml`, `\n[[rig]]\nname = "Handoff source"\nagent = "${agent}"\nenv = { STAMPEDE_FAKE_FIXTURE = "${fixture}", STAMPEDE_FAKE_GATES = "${root}/handoff", CLAUDE_CONFIG_DIR = "${root}/handoff-claude", CODEX_HOME = "${root}/handoff-codex" }\n`);
		await expect.poll(async () => (await (await request.get('/api/rigs')).json()).some((rig: { name: string }) => rig.name === 'Handoff source')).toBe(true);
		const sent: ClientMessage[] = [];
		page.on('websocket', (socket) => socket.on('framesent', ({ payload }) => {
			if (typeof payload === 'string') sent.push(JSON.parse(payload));
		}));
		await openProject(page, 'alpha');
		await page.getByRole('button', { name: 'New Tab' }).click();
		await page.getByRole('menuitem', { name: 'Handoff source', exact: true }).click();
		const source = page.getByRole('tab', { name: /^Handoff source/ });
		const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
		await expect(panel.getByRole('button', { name: 'New Chat from here…' })).toBeDisabled();
		await expect.poll(() => panel.locator('.composer .actions').evaluate((element) => {
			const labels = ['Attach image', 'New Chat from here…', 'Send'];
			const buttons = labels.map((label) => element.querySelector(`button[aria-label="${label}"]`)?.getBoundingClientRect());
			const row = element.getBoundingClientRect();
			return buttons.every((button) => button && Math.abs(button.top - buttons[0]!.top) < 1 && button.left >= row.left && button.right <= row.right)
				&& Math.abs(buttons[2]!.width - 2 * buttons[0]!.width) < 1
				&& Math.abs(buttons[1]!.width - buttons[0]!.width) < 1;
		})).toBe(true);
		await panel.getByRole('textbox', { name: 'Composer' }).fill('Discuss authentication.');
		await panel.getByRole('button', { name: 'Send', exact: true }).click();
		await expect(panel.getByText('Authentication discussed.', { exact: true })).toBeVisible();
		await expect(source.getByTitle('done')).toBeVisible();
		await panel.getByRole('textbox', { name: 'Composer' }).fill('Keep this source draft');
		await panel.getByRole('button', { name: 'New Chat from here…' }).click();
		const dialog = page.getByRole('dialog', { name: 'New Chat from here' });
		await expect(dialog.getByLabel('Rig', { exact: true })).toHaveValue('Handoff source');
		await expect(dialog.getByLabel('Focus (optional)', { exact: true })).toHaveValue('');
		await expect.poll(() => dialog.evaluate((element) => {
			const rect = element.getBoundingClientRect();
			return rect.left >= 0 && rect.right <= innerWidth && rect.top >= 0 && rect.bottom <= innerHeight;
		})).toBe(true);
		const target = agent === 'claude-code' ? 'Fake Codex' : 'Fake Claude';
		await dialog.getByLabel('Rig', { exact: true }).selectOption(target);
		await dialog.getByLabel('Focus (optional)', { exact: true }).fill('Authentication only');
		await dialog.getByRole('button', { name: 'Prepare handoff' }).click();
		await expect(dialog.getByRole('status')).toHaveText('Preparing handoff in Handoff source…');
		await expect(dialog.getByRole('button', { name: 'Prepare handoff' })).toBeDisabled();
		await expect.poll(() => existsSync(`${root}/handoff/handoff`)).toBe(true);
		unlinkSync(`${root}/handoff/handoff`);
		await expect(dialog).toBeHidden();
		await expect(page.getByRole('tab', { name: new RegExp(`^${target}`) })).toHaveAttribute('aria-selected', 'true');
		await expect(panel.getByRole('textbox', { name: 'Composer' })).toHaveValue(summary);
		await expect(panel.locator('.user')).toHaveCount(0);
		expect(sent.filter((message) => message.type === 'chat.send')).toHaveLength(1);
		expect(sent.find((message) => message.type === 'chat.handoff')).toMatchObject({ rig: target, focus: 'Authentication only' });
		await panel.getByRole('button', { name: 'Source Chat · Handoff source', exact: true }).click();
		await expect(source).toHaveAttribute('aria-selected', 'true');
		await expect(panel.getByRole('textbox', { name: 'Composer' })).toHaveValue('Keep this source draft');
		await panel.getByRole('button', { name: `New Chat · ${target}`, exact: true }).click();
		const edited = `${summary}\n\nStart with the login form.`;
		await panel.getByRole('textbox', { name: 'Composer' }).fill(edited);
		await page.reload();
		await expect(panel.getByRole('textbox', { name: 'Composer' })).toHaveValue(edited);
		await panel.getByRole('button', { name: 'Send', exact: true }).click();
		await expect.poll(() => sent.filter((message) => message.type === 'chat.send').length).toBe(2);
		expect(sent.filter((message) => message.type === 'chat.send').at(-1)).toMatchObject({ text: edited });
		await expect(panel.getByRole('textbox', { name: 'Composer' })).toHaveValue('');
		await expect(page.getByRole('tab', { name: new RegExp(`^${target}`) }).getByTitle('done')).toBeVisible();
		await page.reload();
		await expect(panel.getByRole('textbox', { name: 'Composer' })).toHaveValue('');
	});
}
