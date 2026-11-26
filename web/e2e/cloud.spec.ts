import { expect, test } from './fixtures.ts';
import { expectSelected, newTab, openProject, screen } from './shell.ts';
import { readdirSync } from 'node:fs';

test('a Cloud Host worktree created from New worktree runs its Cloud Setup on its own Host, held only while focused', { tag: '@desktop' }, async ({ page }) => {
	test.setTimeout(60_000);
	await page.clock.install();
	const open = new Set<string>();
	page.on('websocket', (socket) => {
		const { pathname } = new URL(socket.url());
		if (!pathname.startsWith('/cloud/')) return;
		open.add(pathname);
		socket.on('close', () => open.delete(pathname));
	});
	await openProject(page, 'alpha');
	const rail = page.getByRole('navigation', { name: 'Projects' });
	const parent = rail.getByRole('button', { name: 'alpha', exact: true });
	await parent.click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'New worktree…' }).click();
	const dialog = page.getByRole('dialog', { name: 'New worktree' });
	await dialog.getByRole('textbox', { name: 'New branch' }).fill('try/cloud');
	await dialog.getByRole('radio', { name: 'Cloud Host' }).click();
	await dialog.getByRole('button', { name: 'Create' }).click();
	await expect(dialog).toBeHidden({ timeout: 30_000 });
	const tile = rail.getByRole('group', { name: 'Worktrees of alpha' }).getByRole('button', { name: /^try\/cloud, on a Cloud Host/ });
	await expect(tile).toHaveAttribute('aria-current', 'true');
	await expect.poll(() => [...open]).toEqual([expect.stringMatching(/^\/cloud\/stampede-[^/]+\/ws$/)]);
	const host = [...open][0].split('/')[2];
	await expectSelected(page.getByRole('tablist', { name: 'Tabs', exact: true }).getByRole('tab', { name: 'Cloud Setup' }));
	await expect(tile).not.toHaveAccessibleName(/setting up/);
	await expect(page.getByRole('status').filter({ hasText: 'Cloud Setup' })).toBeHidden();
	await expect(page.getByText('Reconnecting…')).toBeHidden();
	await expect(page.getByRole('radiogroup', { name: 'Sections' }).getByRole('radio')).toHaveText(['Files', 'Git', 'Compare']);
	await newTab(page, 'Terminal');
	await expect.poll(() => screen(page), { timeout: 15_000 }).toMatch(/\S/);
	await page.keyboard.type('echo $HOME');
	await page.keyboard.press('Enter');
	const home = `${process.env.STAMPEDE_E2E_ROOT}/sprites/${host}/home`;
	await expect.poll(async () => (await screen(page)).replaceAll('\n', '')).toContain(home);
	expect(readdirSync(home)).toContain('alpha');

	await parent.click();
	await page.clock.fastForward(29_000);
	expect([...open]).toEqual([`/cloud/${host}/ws`]);
	await page.clock.fastForward(2_000);
	await expect.poll(() => [...open]).toEqual([]);
	await expect(page.getByText('Reconnecting…')).toBeHidden();

	await tile.click();
	await expect.poll(async () => (await screen(page)).replaceAll('\n', '')).toContain(home);
});
