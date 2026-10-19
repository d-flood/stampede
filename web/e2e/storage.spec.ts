import { expect, test } from './fixtures.ts';
import { execFileSync } from 'node:child_process';
import { appendFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { openProject, showSidebar } from './shell.ts';

test('low storage pauses new Tickets and reports the reason', async ({ page }, { project }) => {
	const parent = process.env.STAMPEDE_E2E_ROOT!;
	const slug = `storage-${project.name}`;
	const root = `${parent}/projects/${slug}`;
	mkdirSync(root);
	const git = (...args: string[]) => execFileSync('git', ['-C', root, '-c', 'user.name=test', '-c', 'user.email=test@example.invalid', '-c', 'commit.gpgsign=false', ...args]);
	git('init', '-q');
	writeFileSync(`${root}/README`, 'Storage\n');
	git('add', 'README');
	git('commit', '-qm', 'Fixture');
	const cli = (input: string, ...args: string[]) => execFileSync(process.env.STAMPEDE_E2E_BINARY!, [...args, '--data-dir', `${parent}/data`], { cwd: root, input });
	cli('# Storage\n', 'epic', 'add', slug, '--title', 'Storage', '--brief', '-');
	cli('Work\n', 'ticket', 'add', slug, '1', '--title', 'Work', '--body', '-');
	cli('', 'epic', 'set-setup', slug, '');
	appendFileSync(`${parent}/config.toml`, '\n[storage]\nmin_free_gib = 1000000\n');
	await expect.poll(async () => {
		const response = await page.request.get('/api/config/file');
		return (await response.json()).content;
	}).toContain('min_free_gib = 1000000');
	await openProject(page, slug);
	const sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'Epics', exact: true }).click();
	await sidebar.getByRole('region', { name: 'Epics' }).getByRole('button', { name: new RegExp(`^${slug}`) }).click();
	const panel = page.getByRole('tabpanel', { name: `Epic ${slug}` });
	const run = panel.getByRole('region', { name: 'Run', exact: true });
	await run.getByRole('combobox', { name: 'Rig' }).selectOption('Fake Claude');
	await run.getByRole('button', { name: 'Start Run' }).click();
	const status = run.getByRole('status').filter({ hasText: 'Low disk space' });
	await expect(run).toContainText('Run paused');
	await expect(status).toContainText('Low disk space');
	await expect(status).toContainText('active Chats continue');
	await expect.poll(() => status.evaluate((element) => {
		const bounds = element.getBoundingClientRect();
		return bounds.left >= 0 && bounds.right <= window.innerWidth && element.scrollWidth <= element.clientWidth;
	})).toBe(true);
	expect(existsSync(`${parent}/data/worktrees/${slug}/integration`)).toBe(false);
	expect(readFileSync(`${parent}/data/epics/${slug}.jsonl`, 'utf8')).not.toContain('ticket.claimed');
	await page.screenshot({ path: test.info().outputPath('storage-paused.png') });
});
