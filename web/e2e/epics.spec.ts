import { expect, test } from './fixtures.ts';
import type { Locator, Page } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { openProject, showSidebar } from './shell.ts';

const stampede = process.env.STAMPEDE_E2E_BINARY ?? fileURLToPath(new URL('../../target/debug/stampede', import.meta.url));

test('an Epic made through the CLI shows live in the Epics section, its Tab and its Tickets', async ({ page }, { project }) => {
	const phone = project.name === 'pixel';
	const slug = `epics-${project.name}`;
	const root = `${process.env.STAMPEDE_E2E_ROOT}/projects/${slug}`;
	mkdirSync(root, { recursive: true });
	execFileSync('git', ['-C', root, 'init', '-q']);
	const cli = (stdin: string, ...args: string[]) =>
		execFileSync(stampede, [...args, '--data-dir', `${process.env.STAMPEDE_E2E_ROOT}/data`], { cwd: root, input: stdin });

	await openProject(page, slug);
	const sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'Epics', exact: true }).click();
	const section = sidebar.getByRole('region', { name: 'Epics' });
	await expect(section).toContainText('No Epics yet');

	cli('# Everywhere\n\nShow Epics on every Device.\n', 'epic', 'add', slug, '--title', 'Epics everywhere', '--brief', '-');
	cli('Build the section.\n', 'ticket', 'add', slug, '1', '--title', 'The section', '--body', '-');
	const card = section.getByRole('button', { name: new RegExp(`^${slug}`) });
	await expect(card).toContainText('Epics everywhere');
	await expect(card).toContainText('0 of 1 resolved');
	cli('Open it as a **Tab**.\n', 'ticket', 'add', slug, '2', '--title', 'The tab', '--blocked-by', '1', '--body', '-');
	await expect(card).toContainText('0 of 2 resolved');
	await card.click();

	const tab = page.getByRole('tablist', { name: 'Tabs', exact: true }).getByRole('tab', { name: `Epic ${slug}` });
	await expect(tab).toHaveAttribute('aria-selected', 'true');
	const panel = page.getByRole('tabpanel', { name: `Epic ${slug}` });
	const tickets = panel.getByRole('region', { name: 'Tickets' });
	await expect(tickets.getByRole('button', { name: /^\d/ })).toHaveText([/^1\s*The section/, /^2\s*The tab/]);
	await expect(tickets.getByRole('button', { name: /^2\s*The tab/ })).toContainText('not started');
	await expect(panel).toContainText('2 not started');
	cli('Show the detail.\n', 'ticket', 'add', slug, '3', '--title', 'The detail', '--blocked-by', '1,2', '--body', '-');
	await expect(tickets.getByRole('button', { name: /^3\s*The detail/ })).toBeVisible();
	cli('', 'note', `${slug}/2`, 'Kept the Tab read-only.');
	cli('', 'epic', 'set-setup', slug, 'pnpm install');

	if (phone) {
		await panel.getByRole('button', { name: /^Setup/ }).click();
		await expect(panel.getByRole('heading', { name: 'Setup Command' })).toBeVisible();
		await expect(panel.getByText('pnpm install')).toBeVisible();
		await panel.getByRole('button', { name: `Back to ${slug}` }).click();
		await panel.getByRole('button', { name: /^Brief/ }).click();
		await expect(panel.getByText('Show Epics on every Device.')).toBeVisible();
		await panel.getByRole('button', { name: `Back to ${slug}` }).click();
	} else {
		await expect(panel.getByRole('region', { name: 'Brief' })).toContainText('Show Epics on every Device.');
		await expect(panel.getByRole('region', { name: 'Setup Command' })).toContainText('pnpm install');
	}

	const row = tickets.getByRole('button', { name: /^2\s*The tab/ });
	await row.click();
	if (phone) {
		await expect(panel.getByRole('heading', { name: `${slug}/2` })).toBeVisible();
		await expect(panel.getByRole('heading', { name: 'The tab' })).toBeVisible();
		await expect(panel.getByText('after 1', { exact: true })).toBeVisible();
	} else {
		await expect(row).toHaveAttribute('aria-expanded', 'true');
	}
	await expect(panel.getByRole('region', { name: 'Body' }).locator('strong', { hasText: 'Tab' })).toBeVisible();
	const notes = panel.getByRole('region', { name: 'Notes' });
	await expect(notes).toContainText('Kept the Tab read-only.');
	await expect(notes).toContainText('Agent');
	cli('', 'note', `${slug}/2`, 'A second Note arrives live.');
	await expect(notes).toContainText('A second Note arrives live.');

	if (phone) await panel.getByRole('button', { name: `Back to ${slug}` }).click();
	else await row.click();
	await expect(notes).toHaveCount(0);
	await expect(tickets.getByRole('button', { name: /^\d/ })).toHaveCount(3);
	await showFiles(await showSidebar(page));
});

// A new Device starts from the last closed Device's layout, so other specs expect the Files section.
async function showFiles(sidebar: Locator) {
	await sidebar.getByRole('button', { name: 'Files', exact: true }).click();
	await expect(sidebar.getByRole('button', { name: 'Files', exact: true })).toHaveAttribute('aria-pressed', 'true');
}

async function epicTab(page: Page, slug: string, ...tickets: [string, string, ...string[]][]) {
	const root = `${process.env.STAMPEDE_E2E_ROOT}/projects/${slug}`;
	mkdirSync(root, { recursive: true });
	execFileSync('git', ['-C', root, 'init', '-q']);
	const cli = (stdin: string, ...args: string[]) =>
		execFileSync(stampede, [...args, '--data-dir', `${process.env.STAMPEDE_E2E_ROOT}/data`], { cwd: root, input: stdin });
	cli('# Plan\n\nEdited from the Epic tab.\n', 'epic', 'add', slug, '--title', 'Edited by David', '--brief', '-');
	for (const [index, [title, body, ...blockedBy]] of tickets.entries()) {
		const blockers = blockedBy.length ? ['--blocked-by', blockedBy.join(',')] : [];
		cli(body, 'ticket', 'add', slug, String(index + 1), '--title', title, ...blockers, '--body', '-');
	}
	await openProject(page, slug);
	const sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'Epics', exact: true }).click();
	await sidebar.getByRole('region', { name: 'Epics' }).getByRole('button', { name: new RegExp(`^${slug}`) }).click();
	return page.getByRole('tabpanel', { name: `Epic ${slug}` });
}

async function openTicket(panel: Locator, phone: boolean, number: number) {
	const row = panel.getByRole('region', { name: 'Tickets' }).getByRole('button', { name: new RegExp(`^${number}\\s`) });
	await row.click();
	if (!phone) await expect(row).toHaveAttribute('aria-expanded', 'true');
}

test('Ticket tags and rigs can be chosen independently and persist on desktop and phone', async ({ page }, { project }) => {
	const phone = project.name === 'pixel';
	const slug = `tags-${project.name}`;
	const panel = await epicTab(page, slug, ['Routine work', 'Build it.\n'], ['Design work', 'Design it.\n', '1']);
	const root = `${process.env.STAMPEDE_E2E_ROOT}/projects/${slug}`;
	const cli = (...args: string[]) => execFileSync(stampede, [...args, '--data-dir', `${process.env.STAMPEDE_E2E_ROOT}/data`], { cwd: root });
	cli('ticket', 'set-tags', `${slug}/1`, 'routine');
	cli('ticket', 'set-tags', `${slug}/2`, 'judgment,frontend-design');
	const tickets = panel.getByRole('region', { name: 'Tickets' });
	const first = tickets.getByRole('combobox', { name: 'Rig for ticket 1', exact: true });
	const second = tickets.getByRole('combobox', { name: 'Rig for ticket 2', exact: true });
	await expect(first).toHaveValue('');
	await expect(second).toHaveValue('');
	await expect(tickets.locator('.tags')).toHaveText(['Routine', 'JudgmentFrontend design']);
	await first.selectOption('Fake Claude');
	await second.selectOption('Fake Codex');
	await expect(first).toBeEnabled();
	await expect(second).toBeEnabled();
	await expect(panel.getByRole('region', { name: 'Body' })).toHaveCount(0);
	await page.reload();
	await expect(first).toHaveValue('Fake Claude');
	await expect(second).toHaveValue('Fake Codex');
	await expect(tickets.locator('.tags')).toHaveText(['Routine', 'JudgmentFrontend design']);
	await expect.poll(() => tickets.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
	await expect.poll(async () => {
		const bounds = await second.boundingBox();
		const viewport = page.viewportSize();
		return !!bounds && !!viewport && bounds.width > 0 && bounds.x >= 0 && bounds.x + bounds.width <= viewport.width;
	}).toBe(true);
	await second.selectOption('');
	await expect(second).toBeEnabled();
	await page.reload();
	await expect(first).toHaveValue('Fake Claude');
	await expect(second).toHaveValue('');
	await openTicket(panel, phone, 2);
	const tags = panel.getByRole('group', { name: 'Tags', exact: true });
	await expect(tags.getByRole('button', { name: 'Routine', exact: true })).toHaveAttribute('aria-pressed', 'false');
	await tags.getByRole('button', { name: 'Routine', exact: true }).click();
	await expect(tags.getByRole('button', { name: 'Routine', exact: true })).toHaveAttribute('aria-pressed', 'true');
	await page.reload();
	await openTicket(panel, phone, 2);
	await expect(tags.getByRole('button', { name: 'Routine', exact: true })).toHaveAttribute('aria-pressed', 'true');
	for (const name of ['Routine', 'Judgment', 'Frontend design']) {
		await tags.getByRole('button', { name, exact: true }).click();
		await expect(tags.getByRole('button', { name, exact: true })).toHaveAttribute('aria-pressed', 'false');
	}
	if (phone) await panel.getByRole('button', { name: `Back to ${slug}` }).click();
	else await tickets.getByRole('button', { name: /^2\s*Design work/ }).click();
	await expect(tickets.locator('.tags').nth(1)).toBeEmpty();
	await expect(second).toHaveValue('');
	const invalid = await page.request.post(`/api/epics/${slug}`, { data: { kind: 'ticket.tags.set', ticket: 1, tags: ['other'] } });
	expect(invalid.status()).toBe(422);
	await showFiles(await showSidebar(page));
});

test('a Ticket body and David’s Note, edited in the Epic tab, persist across a reload', { tag: '@desktop' }, async ({ page }) => {
	const slug = 'edits';
	const panel = await epicTab(page, slug, ['The store', 'Build the store.\n'], ['The tab', 'Show the tab.\n', '1']);
	await openTicket(panel, false, 2);
	await panel.getByRole('region', { name: 'Tickets' }).getByRole('button', { name: 'Edit', exact: true }).click();
	await expect(panel.getByRole('heading', { name: `${slug}/2` })).toBeVisible();
	const editor = panel.getByRole('textbox', { name: 'Body' });
	await expect(editor).toHaveText('Show the tab.');
	await editor.click();
	await page.keyboard.press('ControlOrMeta+a');
	await page.keyboard.type('Show the tab, edited as Markdown.');
	await panel.getByRole('button', { name: 'Save', exact: true }).click();
	const body = panel.getByRole('region', { name: 'Body' });
	await expect(body).toHaveText('Show the tab, edited as Markdown.');

	const note = panel.getByRole('textbox', { name: 'Add a Note' });
	await note.fill('Decided by David.');
	await note.press('Enter');
	const notes = panel.getByRole('region', { name: 'Notes' });
	await expect(notes).toContainText('David · ');
	await expect(notes).toContainText('Decided by David.');
	await expect(note).toHaveValue('');

	await expect.poll(() => page.evaluate(async (slug) => {
		const layout = await (await fetch(`/api/devices/${sessionStorage.getItem('deviceId')}/layout`)).json();
		return Object.values(layout?.projects ?? {}).some((project) =>
			(project as { tabs: { kind: string; slug?: string }[] }).tabs.some((tab) => tab.kind === 'epic' && tab.slug === slug));
	}, slug)).toBe(true);
	await showFiles(await showSidebar(page));
	await page.reload();
	await openTicket(panel, false, 2);
	await expect(body).toHaveText('Show the tab, edited as Markdown.');
	await expect(notes).toContainText('Decided by David.');
	await expect(notes).toContainText('David · ');
	const log = readFileSync(`${process.env.STAMPEDE_E2E_ROOT}/data/epics/${slug}.jsonl`, 'utf8').trim().split('\n').slice(-2);
	expect(log.map((line) => ({ ...JSON.parse(line), at: 0 }))).toEqual([
		{ at: 0, kind: 'ticket.body.set', ticket: 2, body: 'Show the tab, edited as Markdown.' },
		{ at: 0, kind: 'note.added', ticket: 2, author: 'david', text: 'Decided by David.' }
	]);
	await showFiles(await showSidebar(page));
});

test('David changes Blockers, adds a Ticket, edits the Brief and Setup Command, and archives the Epic', async ({ page }, { project }) => {
	const phone = project.name === 'pixel';
	const slug = `plans-${project.name}`;
	const panel = await epicTab(page, slug, ['The store', 'Build the store.\n'], ['The tab', 'Show the tab.\n', '1']);
	const tickets = panel.getByRole('region', { name: 'Tickets' });
	const back = panel.getByRole('button', { name: `Back to ${slug}` });

	await openTicket(panel, phone, 1);
	const blockers = panel.getByRole('region', { name: 'Blockers' }).getByRole('group', { name: 'Blocked by' });
	await blockers.getByRole('button', { name: '2', exact: true }).click();
	await expect(panel.getByRole('alert')).toHaveText(
		`those Blockers would make a cycle: ${slug}/1 is blocked by ${slug}/2, which is blocked by ${slug}/1`
	);
	await expect(blockers.getByRole('button', { name: '2', exact: true })).toHaveAttribute('aria-pressed', 'false');
	if (phone) await back.click();
	await openTicket(panel, phone, 2);
	await blockers.getByRole('button', { name: '1', exact: true }).click();
	await expect(blockers.getByRole('button', { name: '1', exact: true })).toHaveAttribute('aria-pressed', 'false');
	if (phone) await back.click();

	await tickets.getByRole('button', { name: 'Add a Ticket' }).click();
	await expect(panel.getByRole('heading', { name: `${slug}/3` })).toBeVisible();
	const add = panel.getByRole('button', { name: 'Add', exact: true });
	await expect(add).toBeDisabled();
	await panel.getByRole('textbox', { name: 'Title' }).fill('The detail');
	await panel.getByRole('group', { name: 'Blocked by' }).getByRole('button', { name: '2', exact: true }).click();
	await panel.getByRole('textbox', { name: 'Body' }).click();
	await page.keyboard.type('Show the detail.');
	await add.click();
	await expect(tickets.getByRole('button', { name: /^3\s/ })).toContainText(phone ? 'not started · after 2' : 'The detail');

	if (phone) await panel.getByRole('button', { name: /^Setup/ }).click();
	const setup = phone ? panel : panel.getByRole('region', { name: 'Setup Command' });
	await setup.getByRole('button', { name: 'Edit', exact: true }).click();
	await panel.getByRole('textbox', { name: 'Setup Command' }).fill('pnpm --dir web install');
	await panel.getByRole('textbox', { name: 'Setup Command' }).press('Enter');
	await expect(setup.getByText('pnpm --dir web install', { exact: true })).toBeVisible();
	if (phone) await back.click();

	if (phone) await panel.getByRole('button', { name: /^Brief/ }).click();
	const brief = phone ? panel : panel.getByRole('region', { name: 'Brief' });
	await brief.getByRole('button', { name: 'Edit', exact: true }).click();
	const editor = panel.getByRole('textbox', { name: 'Brief' });
	await editor.click();
	await page.keyboard.press('ControlOrMeta+End');
	await page.keyboard.type('\nRevised by David.');
	await panel.getByRole('button', { name: 'Save', exact: true }).click();
	await expect(phone ? panel : brief).toContainText('Revised by David.');
	if (phone) await back.click();

	await panel.getByRole('button', { name: 'Archive Epic' }).click();
	const dialog = page.getByRole('dialog', { name: `Archive ${slug}?` });
	await dialog.getByRole('button', { name: 'Archive', exact: true }).click();
	await expect(page.getByRole('tablist', { name: 'Tabs', exact: true }).getByRole('tab', { name: `Epic ${slug}` })).toHaveCount(0);
	const sidebar = await showSidebar(page);
	const section = sidebar.getByRole('region', { name: 'Epics' });
	const log = readFileSync(`${process.env.STAMPEDE_E2E_ROOT}/data/epics/${slug}.jsonl`, 'utf8');
	expect(log.trim().split('\n').map((line) => JSON.parse(line).kind).slice(3)).toEqual([
		'blockers.set',
		'ticket.added',
		'setup.set',
		'brief.set',
		'epic.archived'
	]);
	const archived = section.getByRole('checkbox', { name: 'Show archived' });
	const card = section.getByRole('button', { name: new RegExp(`^${slug}\\s`) });
	await expect(card).toHaveCount(0);
	await archived.check();
	await expect(card).toContainText('Archived');
	await expect.poll(() => section.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
	await card.click();
	await expect(panel.getByRole('heading', { name: slug, exact: true })).toBeVisible();
	await expect(panel.getByRole('button', { name: 'Archive Epic' })).toHaveCount(0);
	const root = `${process.env.STAMPEDE_E2E_ROOT}/projects/${slug}`;
	execFileSync(stampede, ['epic', 'add', `${slug}-active`, '--title', 'Active plan', '--brief', '-', '--data-dir', `${process.env.STAMPEDE_E2E_ROOT}/data`], { cwd: root, input: '# Active\n' });
	const visible = await showSidebar(page);
	const epics = visible.getByRole('region', { name: 'Epics' });
	await expect(epics.getByRole('button', { name: new RegExp(`^${slug}-active\\s`) })).toBeVisible();
	await epics.getByRole('checkbox', { name: 'Show archived' }).uncheck();
	await expect(card).toHaveCount(0);
	await expect(page.getByRole('tab', { name: `Epic ${slug}`, exact: true })).toHaveAttribute('aria-selected', 'true');
	await epics.getByRole('checkbox', { name: 'Show archived' }).check();
	await card.click();
	await page.reload();
	await expect(panel.getByRole('heading', { name: slug, exact: true })).toBeVisible();
	await openTicket(panel, phone, 1);
	await expect(panel.getByRole('region', { name: 'Body' })).toContainText('Build the store.');
	await showFiles(await showSidebar(page));
});

test('Start on a Frontier Ticket works it with a fake Agent until it is merged', { tag: '@desktop' }, async ({ page }) => {
	const slug = 'start';
	const data = `${process.env.STAMPEDE_E2E_ROOT}/data`;
	const root = `${process.env.STAMPEDE_E2E_ROOT}/projects/${slug}`;
	mkdirSync(root, { recursive: true });
	const git = (...args: string[]) => execFileSync('git', ['-C', root, ...args]);
	git('init', '-q');
	git('config', 'user.name', 'stampede');
	git('config', 'user.email', 'e2e@stampede.invalid');
	git('config', 'commit.gpgsign', 'false');
	writeFileSync(`${root}/README`, 'start\n');
	git('add', 'README');
	git('commit', '-qm', 'Initial commit');
	const cli = (stdin: string, ...args: string[]) => execFileSync(stampede, [...args, '--data-dir', data], { cwd: root, input: stdin });
	cli('# Start\n', 'epic', 'add', slug, '--title', 'Start one', '--brief', '-');
	cli('Write **work.txt**.\n', 'ticket', 'add', slug, '1', '--title', 'The work', '--body', '-');
	cli('Then more.\n', 'ticket', 'add', slug, '2', '--title', 'More', '--blocked-by', '1', '--body', '-');

	await openProject(page, slug);
	const sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'Epics', exact: true }).click();
	await sidebar.getByRole('region', { name: 'Epics' }).getByRole('button', { name: new RegExp(`^${slug}`) }).click();
	const panel = page.getByRole('tabpanel', { name: `Epic ${slug}` });
	const tickets = panel.getByRole('region', { name: 'Tickets' });
	const row = tickets.getByRole('button', { name: /^1\s*The work/ });
	await row.click();
	const start = panel.getByRole('button', { name: 'Start', exact: true });
	await expect(start).toBeDisabled();
	cli('', 'epic', 'set-setup', slug, '');
	await expect(start).toBeEnabled();
	await tickets.getByRole('combobox', { name: 'Rig', exact: true }).selectOption('Fake Claude Verdict');
	await start.click();

	await expect(row).toContainText('merged', { timeout: 15000 });
	await expect(start).toHaveCount(0);
	expect(readFileSync(`${data}/worktrees/${slug}/integration/work.txt`, 'utf8')).toBe('Done by the fake.\n');
	await expect.poll(() => existsSync(`${data}/worktrees/${slug}/1`)).toBe(false);

	await expect(page.getByRole('tab', { name: /^Fake Claude Verdict/ })).toHaveCount(0);

	await page.getByRole('tab', { name: `Epic ${slug}` }).click();
	await row.click();
	await tickets.getByRole('button', { name: /^2\s*More/ }).click();
	await expect(start).toBeEnabled();

	await panel.getByRole('button', { name: /^Land into / }).click();
	const confirm = page.getByRole('dialog', { name: /^Land start / });
	await expect(confirm).toContainText('Still unresolved:');
	await expect(confirm.getByRole('listitem')).toHaveText([/^2\s*More · not started$/]);
	await confirm.getByRole('button', { name: 'Cancel', exact: true }).click();
	await expect(confirm).toHaveCount(0);
	await showFiles(await showSidebar(page));
});

test('a merged Ticket moved back to not started is worked again on the Epic branch, and an edit mid-Chat is marked', async ({ page }, { project }) => {
	const phone = project.name === 'pixel';
	const slug = `reopen-${project.name}`;
	const data = `${process.env.STAMPEDE_E2E_ROOT}/data`;
	const root = `${process.env.STAMPEDE_E2E_ROOT}/projects/${slug}`;
	mkdirSync(root, { recursive: true });
	const git = (...args: string[]) => execFileSync('git', ['-C', root, ...args]);
	git('init', '-q');
	git('config', 'user.name', 'stampede');
	git('config', 'user.email', 'e2e@stampede.invalid');
	git('config', 'commit.gpgsign', 'false');
	writeFileSync(`${root}/README`, 'reopen\n');
	git('add', 'README');
	git('commit', '-qm', 'Initial commit');
	const cli = (stdin: string, ...args: string[]) => execFileSync(stampede, [...args, '--data-dir', data], { cwd: root, input: stdin });
	cli('# Reopen\n', 'epic', 'add', slug, '--title', 'Reopen one', '--brief', '-');
	cli('Write **work.txt**.\n', 'ticket', 'add', slug, '1', '--title', 'The work', '--body', '-');
	cli('', 'epic', 'set-setup', slug, '');

	await openProject(page, slug);
	const sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'Epics', exact: true }).click();
	await sidebar.getByRole('region', { name: 'Epics' }).getByRole('button', { name: new RegExp(`^${slug}`) }).click();
	const panel = page.getByRole('tabpanel', { name: `Epic ${slug}` });
	const row = panel.getByRole('region', { name: 'Tickets' }).getByRole('button', { name: /^1\s*The work/ });
	await row.click();
	const start = panel.getByRole('button', { name: 'Start', exact: true });
	const rig = (phone ? panel : panel.getByRole('region', { name: 'Tickets' })).getByRole('combobox', { name: 'Rig', exact: true });
	const reopen = panel.getByRole('button', { name: 'Move to not started' });
	await expect(reopen).toHaveCount(0);
	await rig.selectOption('Fake Claude Verdict');
	await start.click();
	const status = phone ? panel.locator('.chips') : row;
	await expect(status).toContainText('merged', { timeout: 15000 });
	const log = () => readFileSync(`${data}/epics/${slug}.jsonl`, 'utf8').trim().split('\n').map((line) => JSON.parse(line));
	await expect.poll(() => log().at(-1).kind).toBe('worktree.removed');

	await reopen.click();
	await expect(status).toContainText('not started');
	await expect(reopen).toHaveCount(0);
	await expect(start).toBeEnabled();
	await rig.selectOption('Fake Claude');
	await start.click();
	await expect(status).toContainText('in progress', { timeout: 15000 });
	const claims = log().filter((entry) => entry.kind === 'ticket.claimed');
	expect(claims).toHaveLength(2);
	expect(readFileSync(`${claims[1].worktree}/work.txt`, 'utf8')).toBe('Done by the fake.\n');

	const marker = panel.getByText('edited since its Chat started', { exact: true });
	await expect(marker).toHaveCount(0);
	await (phone ? panel : panel.getByRole('region', { name: 'Tickets' })).getByRole('button', { name: 'Edit', exact: true }).click();
	await panel.getByRole('textbox', { name: 'Body' }).click();
	await page.keyboard.press('ControlOrMeta+a');
	await page.keyboard.type('Write work.txt, then say so.');
	await panel.getByRole('button', { name: 'Save', exact: true }).click();
	await expect(marker).toBeVisible();
	await expect(status).toContainText('in progress');
	if (phone) {
		await panel.getByRole('button', { name: `Back to ${slug}` }).click();
		await expect(row).toContainText('in progress · Fake Claude');
		await expect(row).toContainText('· edited');
	}
	expect(log().map((entry) => entry.kind).slice(-5)).toEqual(['worktree.removed', 'ticket.reopened', 'ticket.rig.set', 'ticket.claimed', 'ticket.body.set']);
	await showFiles(await showSidebar(page));
});

test('a Run started from the Epic tab works the Epic through and calls from the Attention list when it finishes', { tag: '@desktop' }, async ({ page }) => {
	const slug = 'run';
	const data = `${process.env.STAMPEDE_E2E_ROOT}/data`;
	const root = `${process.env.STAMPEDE_E2E_ROOT}/projects/${slug}`;
	const gate = `${process.env.STAMPEDE_E2E_ROOT}/${slug}.go`;
	mkdirSync(root, { recursive: true });
	const git = (...args: string[]) => execFileSync('git', ['-C', root, ...args]);
	git('init', '-q');
	git('config', 'user.name', 'stampede');
	git('config', 'user.email', 'e2e@stampede.invalid');
	git('config', 'commit.gpgsign', 'false');
	writeFileSync(`${root}/README`, 'run\n');
	git('add', 'README');
	git('commit', '-qm', 'Initial commit');
	const cli = (stdin: string, ...args: string[]) => execFileSync(stampede, [...args, '--data-dir', data], { cwd: root, input: stdin });
	cli('# Run\n', 'epic', 'add', slug, '--title', 'Run it', '--brief', '-');
	cli('Write **work.txt**.\n', 'ticket', 'add', slug, '1', '--title', 'The work', '--body', '-');
	cli('Then more.\n', 'ticket', 'add', slug, '2', '--title', 'More', '--blocked-by', '1', '--body', '-');
	cli('Beside it.\n', 'ticket', 'add', slug, '3', '--title', 'Beside', '--body', '-');

	await openProject(page, slug);
	const sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'Epics', exact: true }).click();
	await sidebar.getByRole('region', { name: 'Epics' }).getByRole('button', { name: new RegExp(`^${slug}`) }).click();
	const panel = page.getByRole('tabpanel', { name: `Epic ${slug}` });
	const run = panel.getByRole('region', { name: 'Run', exact: true });
	const startRun = run.getByRole('button', { name: 'Start Run' });
	await expect(startRun).toBeDisabled();
	cli('', 'epic', 'set-setup', slug, `until [ -e ${gate} ]; do sleep 0.1; done`);
	await expect(startRun).toBeEnabled();
	await run.getByRole('combobox', { name: 'Rig' }).selectOption('Fake Claude Verdict');
	const limit = run.getByRole('group', { name: 'Parallel' });
	await expect(limit.getByRole('button', { name: 'Fewer' })).toBeDisabled();
	await limit.getByRole('button', { name: 'More' }).click();
	await expect(limit.locator('output')).toHaveText('2');
	await startRun.click();

	await expect(run).toContainText('Run live');
	await expect(startRun).toHaveCount(0);
	const land = panel.getByRole('button', { name: /^Land( into .+)?$/ });
	await expect(land).toBeDisabled();
	await expect(run.getByRole('combobox', { name: 'Rig' })).toHaveValue('Fake Claude Verdict');
	await limit.getByRole('button', { name: 'More' }).click();
	await expect(limit.locator('output')).toHaveText('3');
	await expect(panel.getByRole('region', { name: 'Tickets' })).not.toContainText('in progress');

	await page.getByRole('button', { name: `Close Epic ${slug}` }).click();
	await expect(panel).toHaveCount(0);
	writeFileSync(gate, '');
	const list = page.getByRole('button', { name: 'Attention list' });
	const entry = page.getByRole('menuitem', { name: `Epic ${slug} in ${slug}` });
	await list.click();
	await expect(entry).toBeVisible({ timeout: 20000 });
	await entry.click();
	await expect(page.getByRole('tab', { name: `Epic ${slug}` })).toHaveAttribute('aria-selected', 'true');
	await expect(panel).toContainText('3 merged');
	await expect(run).not.toContainText('Run live');
	await expect(startRun).toBeVisible();
	expect(readFileSync(`${data}/worktrees/${slug}/integration/work.txt`, 'utf8')).toBe('Done by the fake.\n');
	await list.click();
	await expect(entry).toHaveCount(0);
	await page.keyboard.press('Escape');

	const base = git('symbolic-ref', '--short', 'HEAD').toString().trim();
	await expect(land).toHaveText(`Land into ${base}`);
	await expect(land).toBeEnabled();
	writeFileSync(`${root}/scratch.txt`, 'Not yet added.\n');
	await land.click();
	const confirm = page.getByRole('dialog', { name: `Land ${slug} into ${base}?` });
	await confirm.getByRole('button', { name: 'Land', exact: true }).click();
	const refusal = `${root} has uncommitted changes in scratch.txt; commit them or put them aside and land again`;
	await expect(confirm.getByRole('alert')).toHaveText(refusal);
	const worktree = panel.getByRole('region', { name: 'Integration Worktree' });
	await expect(worktree).toContainText(`Landing was refused: ${refusal}.`);
	expect(existsSync(`${root}/work.txt`)).toBe(false);
	rmSync(`${root}/scratch.txt`);
	await confirm.getByRole('button', { name: 'Land', exact: true }).click();
	await expect(confirm).toHaveCount(0);
	await expect(worktree).toContainText(`Landed into ${base} at`);
	await expect(land).toBeDisabled();
	expect(readFileSync(`${root}/work.txt`, 'utf8')).toBe('Done by the fake.\n');
	expect(existsSync(`${data}/worktrees/${slug}/integration`)).toBe(false);
	expect(git('branch', '--list', `stampede/${slug}/*`).toString()).toBe('');
	await showFiles(await showSidebar(page));
});

test('a Run with a Reviewer reviews the Epic once its Tickets are done, and its Remediator judges and fixes the Findings', { tag: '@desktop' }, async ({ page }) => {
	const slug = 'review';
	const data = `${process.env.STAMPEDE_E2E_ROOT}/data`;
	const root = `${process.env.STAMPEDE_E2E_ROOT}/projects/${slug}`;
	mkdirSync(root, { recursive: true });
	const git = (...args: string[]) => execFileSync('git', ['-C', root, ...args]);
	git('init', '-q');
	git('config', 'user.name', 'stampede');
	git('config', 'user.email', 'e2e@stampede.invalid');
	git('config', 'commit.gpgsign', 'false');
	writeFileSync(`${root}/README`, 'review\n');
	git('add', 'README');
	git('commit', '-qm', 'Initial commit');
	const cli = (stdin: string, ...args: string[]) => execFileSync(stampede, [...args, '--data-dir', data], { cwd: root, input: stdin });
	cli('# Review\n', 'epic', 'add', slug, '--title', 'Review it', '--brief', '-');
	cli('Write **work.txt**.\n', 'ticket', 'add', slug, '1', '--title', 'The work', '--body', '-');
	cli('', 'epic', 'set-setup', slug, '');

	await openProject(page, slug);
	const sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'Epics', exact: true }).click();
	await sidebar.getByRole('region', { name: 'Epics' }).getByRole('button', { name: new RegExp(`^${slug}`) }).click();
	const panel = page.getByRole('tabpanel', { name: `Epic ${slug}` });
	const run = panel.getByRole('region', { name: 'Run', exact: true });
	await run.getByRole('combobox', { name: 'Rig' }).selectOption('Fake Claude Verdict');
	await expect(run.getByRole('combobox', { name: 'Remediator' })).toHaveCount(0);
	await run.getByRole('button', { name: 'Reviewers: none' }).click();
	await page.getByRole('menuitemcheckbox', { name: 'Fake Reviewer' }).click();
	await expect(page.getByRole('menuitemcheckbox', { name: 'Fake Reviewer' })).toHaveAttribute('aria-checked', 'true');
	await page.keyboard.press('Escape');
	await expect(run.getByRole('button', { name: 'Reviewers: Fake Reviewer' })).toBeVisible();
	await run.getByRole('combobox', { name: 'Remediator' }).selectOption('Fake Remediator');
	await run.getByRole('button', { name: 'Start Run' }).click();

	const review = panel.getByRole('region', { name: 'Review' });
	await expect(review).toContainText('Remediated at', { timeout: 20000 });
	await expect(review.getByRole('listitem')).toHaveText([/Reviewer 1\s*Fake Reviewer\s*reported/, /Remediator\s*Fake Remediator\s*remediated/]);
	const finding = review.locator('details', { hasText: 'The greeting shouts' });
	await expect(finding.locator('summary')).toContainText('Accepted');
	await finding.locator('summary').click();
	await expect(finding).toContainText('work.txt is all capitals.');
	await expect(finding).toContainText('Accepted: It does shout');
	await expect(run).not.toContainText('Run live');
	await expect(run.getByRole('button', { name: 'Start Run' })).toBeVisible();
	expect(readFileSync(`${data}/worktrees/${slug}/integration/work.txt`, 'utf8')).toBe('Remediated.\n');
	expect(existsSync(`${data}/worktrees/${slug}/review-1`)).toBe(false);
	await showFiles(await showSidebar(page));
});

test('a paused Run starts nothing until it is resumed, and stopping it gives its Ticket back', { tag: '@desktop' }, async ({ page }) => {
	const slug = 'pause';
	const data = `${process.env.STAMPEDE_E2E_ROOT}/data`;
	const root = `${process.env.STAMPEDE_E2E_ROOT}/projects/${slug}`;
	mkdirSync(root, { recursive: true });
	const git = (...args: string[]) => execFileSync('git', ['-C', root, ...args]);
	git('init', '-q');
	git('config', 'user.name', 'stampede');
	git('config', 'user.email', 'e2e@stampede.invalid');
	git('config', 'commit.gpgsign', 'false');
	writeFileSync(`${root}/README`, 'pause\n');
	git('add', 'README');
	git('commit', '-qm', 'Initial commit');
	const cli = (cwd: string, stdin: string, ...args: string[]) => execFileSync(stampede, [...args, '--data-dir', data], { cwd, input: stdin });
	cli(root, '# Pause\n', 'epic', 'add', slug, '--title', 'Pause it', '--brief', '-');
	cli(root, 'Write **work.txt**.\n', 'ticket', 'add', slug, '1', '--title', 'The work', '--body', '-');
	cli(root, 'Beside it.\n', 'ticket', 'add', slug, '2', '--title', 'Beside', '--body', '-');
	cli(root, '', 'epic', 'set-setup', slug, '');
	const log = () => readFileSync(`${data}/epics/${slug}.jsonl`, 'utf8').trim().split('\n').map((line) => JSON.parse(line));
	const kinds = (kind: string) => log().filter((entry) => entry.kind === kind);

	await openProject(page, slug);
	const sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'Epics', exact: true }).click();
	await sidebar.getByRole('region', { name: 'Epics' }).getByRole('button', { name: new RegExp(`^${slug}`) }).click();
	const panel = page.getByRole('tabpanel', { name: `Epic ${slug}` });
	const run = panel.getByRole('region', { name: 'Run', exact: true });
	const tickets = panel.getByRole('region', { name: 'Tickets' });
	const row = (number: number) => tickets.getByRole('button', { name: new RegExp(`^${number}\\s`) });
	await run.getByRole('combobox', { name: 'Rig' }).selectOption('Fake Claude');
	await run.getByRole('button', { name: 'Start Run' }).click();
	await expect(row(1)).toContainText('in progress', { timeout: 15000 });

	await run.getByRole('button', { name: 'Pause' }).click();
	await expect(run).toContainText('Run paused');
	await expect(run.getByRole('button', { name: 'Pause' })).toHaveCount(0);
	cli(kinds('ticket.claimed')[0].worktree, '', 'complete', `${slug}/1`, '-m', 'Done by hand');
	await expect(row(1)).toContainText('merged', { timeout: 15000 });
	await page.waitForTimeout(1000);
	expect(kinds('ticket.claimed')).toHaveLength(1);
	await expect(row(2)).toContainText('not started');

	await run.getByRole('button', { name: 'Resume' }).click();
	await expect(run).toContainText('Run live');
	await expect(row(2)).toContainText('in progress', { timeout: 15000 });
	await run.getByRole('button', { name: 'Stop' }).click();
	const dialog = page.getByRole('dialog', { name: 'Stop the Run?' });
	await dialog.getByRole('button', { name: 'Stop' }).click();
	await expect(row(2)).toContainText('not started', { timeout: 15000 });
	await expect(run.getByRole('button', { name: 'Start Run' })).toBeVisible();
	expect(kinds('claim.released')).toHaveLength(1);
	expect(existsSync(kinds('ticket.claimed')[1].worktree)).toBe(true);
	await showFiles(await showSidebar(page));
});

test('a Run’s worktree Projects sit beside their Project, open on a tap, and stay until deleted', async ({ page }, { project }) => {
	const phone = project.name === 'pixel';
	const slug = `nest-${project.name}`;
	const data = `${process.env.STAMPEDE_E2E_ROOT}/data`;
	const root = `${process.env.STAMPEDE_E2E_ROOT}/projects/${slug}`;
	const gate = `${process.env.STAMPEDE_E2E_ROOT}/gated-${project.name}/verdict`;
	mkdirSync(root, { recursive: true });
	const git = (...args: string[]) => execFileSync('git', ['-C', root, ...args]);
	git('init', '-q');
	git('config', 'user.name', 'stampede');
	git('config', 'user.email', 'e2e@stampede.invalid');
	git('config', 'commit.gpgsign', 'false');
	writeFileSync(`${root}/README`, 'nest\n');
	git('add', 'README');
	git('commit', '-qm', 'Initial commit');
	const cli = (stdin: string, ...args: string[]) => execFileSync(stampede, [...args, '--data-dir', data], { cwd: root, input: stdin });
	cli('# Nest\n', 'epic', 'add', slug, '--title', 'Nest them', '--brief', '-');
	cli('Write **work.txt**.\n', 'ticket', 'add', slug, '1', '--title', 'The work', '--body', '-');
	cli('Beside it.\n', 'ticket', 'add', slug, '2', '--title', 'Beside', '--body', '-');
	cli('', 'epic', 'set-setup', slug, '');

	await openProject(page, slug);
	const sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'Epics', exact: true }).click();
	await sidebar.getByRole('region', { name: 'Epics' }).getByRole('button', { name: new RegExp(`^${slug}`) }).click();
	const run = page.getByRole('tabpanel', { name: `Epic ${slug}` }).getByRole('region', { name: 'Run', exact: true });
	await run.getByRole('combobox', { name: 'Rig' }).selectOption(`Fake Claude Gated ${project.name}`);
	await run.getByRole('group', { name: 'Parallel' }).getByRole('button', { name: 'More' }).click();
	await run.getByRole('button', { name: 'Start Run' }).click();
	await expect(run).toContainText('Run live');

	if (phone) await page.getByRole('button', { name: 'Previous screen' }).click();
	const rail = page.getByRole('navigation', { name: 'Projects' });
	const parent = rail.getByRole('button', { name: slug, exact: true });
	const row = rail.getByRole('listitem').filter({ has: page.getByRole('button', { name: slug, exact: true }) });
	const worktrees = row.getByRole('group', { name: `Worktrees of ${slug}` });
	await expect(worktrees.locator('.tile')).toHaveText(['In', '1', '2'], { timeout: 15000 });
	await expect(worktrees.getByRole('button', { name: `Integration ${slug}`, exact: true })).toBeVisible();
	const second = worktrees.getByRole('button', { name: `${slug}/2, working`, exact: true });
	await expect(worktrees.getByRole('button', { name: `${slug}/1, working`, exact: true })).toBeVisible();
	await expect(second).toBeVisible();
	await expect(row).toContainText(phone ? `Run ${slug} · 0 of 2` : '0/2');

	await second.click();
	await expect(second).toHaveAttribute('aria-current', 'true');
	const heading = phone ? page.locator('.phone > header h1') : page.getByRole('complementary', { name: 'Sidebar' }).getByRole('heading', { level: 1 });
	await expect(heading).toHaveText(`${slug}/2`);
	const chat = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	await expect(chat.locator('.user')).toContainText(`/ticket Your Ticket is ${slug}/2`);
	const worktree = await showSidebar(page);
	await expect(worktree.getByText(`stampede/${slug}/2`, { exact: true })).toBeVisible();
	await worktree.getByRole('button', { name: 'Changes', exact: true }).click();
	await expect(worktree.getByRole('region', { name: 'Changes' }).getByRole('button', { name: 'work.txt', exact: true })).toBeVisible();

	if (phone) {
		await page.getByRole('button', { name: 'Previous screen' }).click();
		await page.getByRole('button', { name: 'Previous screen' }).click();
	}
	const integration = worktrees.getByRole('button', { name: `Integration ${slug}`, exact: true });
	await expect.poll(() => (rmSync(gate, { force: true }), worktrees.getByRole('button').count()), { timeout: 20000 }).toBe(1);
	await expect(integration).toBeVisible();
	await expect(row).not.toContainText(phone ? `Run ${slug}` : '/2');
	await expect(parent).toHaveAttribute('aria-current', 'true');
	expect(readFileSync(`${data}/worktrees/${slug}/integration/work.txt`, 'utf8')).toBe('Done by the fake.\n');
	expect(cli('', 'ticket', `${slug}/2`).toString()).toContain('Status: merged');

	await integration.click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'Delete worktree' }).click();
	const dialog = page.getByRole('dialog', { name: `Delete Integration ${slug}?` });
	await expect(dialog).toContainText(`commits no other branch has. Deleting it and stampede/${slug}/epic discards them.`);
	await dialog.getByRole('button', { name: 'Delete', exact: true }).click();
	await expect(worktrees).toHaveCount(0);
	expect(existsSync(`${data}/worktrees/${slug}/integration`)).toBe(false);
	if (phone) await parent.click();
	const completed = page.getByRole('tablist', { name: 'Tabs', exact: true }).getByRole('tab', { name: /^Fake Claude Gated/ });
	await expect(completed).toHaveCount(0);
	await expect(chat).toHaveCount(0);
	await showFiles(await showSidebar(page));
});
