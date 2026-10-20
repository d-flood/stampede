import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from './fixtures.ts';
import { openProject, screen } from './shell.ts';

function call(chatId: string, name: string, args: object) {
	const output = execFileSync(process.env.STAMPEDE_E2E_BINARY!, [
		'chat-tools', '--data-dir', join(process.env.STAMPEDE_E2E_ROOT!, 'data'), '--chat-id', chatId
	], {
		input: `${JSON.stringify({ id: 1, method: 'tools/call', params: { name, arguments: args } })}\n`,
		encoding: 'utf8', timeout: 10_000
	});
	const { result } = JSON.parse(output);
	expect(result.isError, JSON.stringify(result)).toBe(false);
	return result.structuredContent;
}

test('agents stage editable commands, users run them, and hidden output remains readable', async ({ page }, { project }) => {
	let chatId = '';
	page.on('websocket', (socket) => socket.on('framereceived', ({ payload }) => {
		if (typeof payload !== 'string') return;
		const message = JSON.parse(payload);
		if (message.type === 'chat.created') chatId = message.chat.id;
	}));
	await openProject(page, 'alpha');
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude', exact: true }).click();
	await expect.poll(() => chatId).not.toBe('');
	const command = "printf 'unreviewed\\n' > staged-output\nprintf 'second line\\n'";
	const { terminal_id: id } = call(chatId, 'stage_command', { command });
	const tab = page.locator(`[data-tab="${id}"]`);
	await expect(tab).toHaveAttribute('aria-selected', 'true');
	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('button', { name: 'Run', exact: true }) });
	const composer = panel.getByRole('textbox', { name: 'Composer' });
	await expect(composer).toHaveValue(command);
	await expect.poll(() => screen(page)).toMatch(/\S/);
	const output = join(process.env.STAMPEDE_E2E_ROOT!, 'projects/alpha/staged-output');
	expect(existsSync(output)).toBe(false);
	expect(call(chatId, 'read_terminal', { terminal_id: id }).pending).toBe(true);
	await page.reload();
	await expect(composer).toHaveValue(command);
	expect(existsSync(output)).toBe(false);
	await expect.poll(() => panel.evaluate((element) => {
		const bounds = element.getBoundingClientRect();
		return bounds.width > 0 && bounds.right <= innerWidth && element.scrollWidth <= element.clientWidth;
	})).toBe(true);
	await page.screenshot({ path: test.info().outputPath('pending.png') });
	await composer.fill("printf 'edited\\n' > staged-output\nprintf '\\033[31mvisible-result\\033[0m\\n'");
	if (project.name === 'desktop') await composer.press('Enter');
	else await panel.getByRole('button', { name: 'Run', exact: true }).click();
	await expect(page.getByText('Review command before running')).not.toBeVisible();
	await expect.poll(() => existsSync(output) ? readFileSync(output, 'utf8') : '').toBe('edited\n');
	await expect.poll(() => screen(page)).toMatch(/^visible-result$/m);
	await page.locator(`[data-tab="${chatId}"]`).click();
	await expect(tab).toHaveAttribute('aria-selected', 'false');
	const read = call(chatId, 'read_terminal', { terminal_id: id });
	expect(read.pending).toBe(false);
	expect(read.output).toMatch(/^visible-result$/m);
	expect(read.output).not.toContain('\x1b');
});
