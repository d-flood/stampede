import { expect, test } from './fixtures.ts';
import { newTab, openProject, screen } from './shell.ts';
import { readdirSync } from 'node:fs';

test('a Cloud Host worktree runs on its own Host and holds a connection to it only while focused', { tag: '@desktop' }, async ({ page }) => {
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
	const created = await page.request.post(`/api/projects/${await parent.getAttribute('data-project')}/worktrees`, { data: { cloud: 'sprite', branch: 'try/cloud', from: 'HEAD' } });
	expect(created.ok()).toBe(true);
	const { host } = JSON.parse((await created.text()).trim().split('\n').at(-1)!);
	const tile = rail.getByRole('group', { name: 'Worktrees of alpha' }).getByRole('button', { name: 'try/cloud', exact: true });
	expect([...open]).toEqual([]);

	await tile.click();
	await expect(tile).toHaveAttribute('aria-current', 'true');
	await expect(page.getByRole('button', { name: 'New Tab' })).toBeVisible();
	await expect(page.getByText('Reconnecting…')).toBeHidden();
	expect([...open]).toEqual([`/cloud/${host}/ws`]);
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
