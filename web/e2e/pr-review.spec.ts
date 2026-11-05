import { expect, test } from './fixtures.ts';
import type { Page } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { currentTab, showSidebar, openProject } from './shell.ts';

test.use({ reducedMotion: 'reduce' });

const URL = 'https://github.com/acme/widgets.git';
const LIST = Array.from({ length: 30 }, (_, index) => `export const line${index + 1} = ${index + 1};\n`).join('');

function signIn(state: 'OPEN' | 'MERGED', { author = 'octo', fail }: { author?: string; fail?: string } = {}) {
	const pr = {
		number: 7, title: 'Add widgets', author: { login: author }, isDraft: false, updatedAt: '2026-11-01T00:00:00Z', state,
		baseRefName: 'main', url: 'https://github.com/acme/widgets/pull/7', requested: true
	};
	const fixture = { user: 'david', repo: { nameWithOwner: 'acme/widgets', url: 'https://github.com/acme/widgets' }, prs: [pr], fail };
	writeFileSync(`${process.env.STAMPEDE_E2E_ROOT}/gh.json`, JSON.stringify(fixture));
}

/** A repository whose origin, reached through `insteadOf`, carries PR 7 only as `refs/pull/7/head`. */
function repository() {
	const root = `${process.env.STAMPEDE_E2E_ROOT}/projects/widgets`;
	const origin = `${process.env.STAMPEDE_E2E_ROOT}/origin.git`;
	execFileSync('git', ['init', '-q', '--bare', '-b', 'main', origin]);
	mkdirSync(root);
	const git = (...args: string[]) => execFileSync('git', ['-C', root, ...args], { encoding: 'utf8' }).trim();
	git('init', '-q', '-b', 'main');
	for (const [key, value] of [['user.name', 'stampede'], ['user.email', 'e2e@stampede.invalid'], ['commit.gpgsign', 'false']]) git('config', key, value);
	writeFileSync(`${root}/README.md`, 'widgets\n');
	mkdirSync(`${root}/src`);
	writeFileSync(`${root}/src/list.ts`, LIST);
	git('add', '.');
	git('commit', '-qm', 'Initial commit');
	git('remote', 'add', 'origin', URL);
	git('config', `url.${origin}.insteadOf`, URL);
	git('push', '-q', 'origin', 'main');
	const push = (...files: (string | [path: string, text: string])[]) => {
		git('switch', '-q', '--detach', 'main');
		for (const file of files) {
			const [path, text] = typeof file === 'string' ? [file, `export const ${file.split('.')[0]} = true;\n`] : file;
			writeFileSync(`${root}/src/${path}`, text);
		}
		git('add', '.');
		git('commit', '-qm', 'Add widgets');
		git('push', '-qf', 'origin', `${git('rev-parse', 'HEAD')}:refs/pull/7/head`);
		git('switch', '-q', 'main');
	};
	push('widget.ts');
	return { git, push };
}

async function comparePanel(page: Page) {
	const sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'Compare', exact: true }).click();
	return sidebar.getByRole('region', { name: 'Compare' });
}

async function openPr(page: Page) {
	const compare = await comparePanel(page);
	await compare.getByRole('button', { name: 'Pull request…' }).click();
	await page.getByRole('dialog', { name: 'Open a pull request' }).getByRole('option', { name: /Add widgets/ }).click();
	await expect(compare.getByRole('heading')).toHaveText('#7 Add widgets');
	return compare;
}

test('A PR review opens from the picker, refreshes, and closes back to the plain Comparison', async ({ page }) => {
	signIn('OPEN');
	const { git, push } = repository();
	await openProject(page, 'widgets');
	let compare = await comparePanel(page);
	await compare.getByRole('button', { name: 'Pull request…' }).click();
	const dialog = page.getByRole('dialog', { name: 'Open a pull request' });
	const listed = dialog.getByRole('option', { name: /Add widgets/ });
	await expect(listed).toContainText('#7');
	await expect(listed).toContainText('Review requested');
	await listed.click();
	await expect(dialog).toBeHidden();

	await expect(compare.getByRole('heading')).toHaveText('#7 Add widgets');
	await expect(compare.getByRole('link', { name: 'Open on GitHub' })).toHaveAttribute('href', 'https://github.com/acme/widgets/pull/7');
	await expect(compare.getByRole('link', { name: 'Open on GitHub' })).toHaveAttribute('target', '_blank');
	await expect(compare.getByRole('button', { name: /^Target/ })).toHaveCount(0);
	const files = compare.getByRole('list', { name: 'Compared files' });
	await expect(files.getByRole('button', { name: /\.ts$/ })).toHaveCount(1);
	await files.getByRole('button', { name: 'widget.ts', exact: true }).click();
	const diff = page.locator('.diff:not([hidden])');
	await expect(diff.locator('.path')).toHaveText('src/widget.ts');
	await expect(await currentTab(page)).toContainText('#7');
	await expect(diff.locator('.cm-changedLine', { hasText: 'widget = true' })).toBeVisible();
	expect(git('branch', '--format=%(refname:short)')).toBe('main');

	push('gadget.ts');
	compare = await comparePanel(page);
	await compare.getByRole('button', { name: 'Refresh' }).click();
	await expect(files.getByRole('button', { name: 'gadget.ts', exact: true })).toBeVisible();
	await expect(files.getByRole('button', { name: 'widget.ts', exact: true })).toHaveCount(0);

	await compare.getByRole('button', { name: 'Close' }).click();
	await expect(compare.getByRole('button', { name: /^Target/ })).toBeVisible();
	await expect(compare.getByRole('heading')).toHaveCount(0);
	const reviews = compare.getByRole('list', { name: 'PR reviews in progress' });
	await reviews.getByRole('button', { name: /Add widgets/ }).click();
	await expect(compare.getByRole('heading')).toHaveText('#7 Add widgets');

	signIn('MERGED');
	await compare.getByRole('button', { name: 'Refresh' }).click();
	await expect(compare.getByRole('status')).toContainText('#7 was merged on GitHub.');
	await compare.getByRole('button', { name: 'Discard', exact: true }).click();
	await expect(compare.getByRole('button', { name: /^Target/ })).toBeVisible();
	await expect(reviews).toHaveCount(0);
	await expect.poll(() => git('for-each-ref', 'refs/stampede/')).toBe('');
});

test('Without gh signed in, the PR button is disabled and says why', async ({ page }, { project }) => {
	repository();
	await openProject(page, 'widgets');
	const compare = await comparePanel(page);
	const button = compare.getByRole('button', { name: 'Pull request…' });
	await expect(button).toBeDisabled();
	const reason = 'gh is not signed in to github.com; run gh auth login --hostname github.com';
	if (project.name === 'desktop') {
		await button.hover();
		await expect(page.getByRole('tooltip')).toHaveText(reason);
	} else {
		await expect(compare).toContainText(reason);
	}
	await button.click({ force: true });
	await expect(page.getByRole('dialog', { name: 'Open a pull request' })).toHaveCount(0);
});

test('Viewed & next marks the file Viewed and moves to the next file', async ({ page }) => {
	signIn('OPEN');
	const { push } = repository();
	push('gadget.ts', 'widget.ts');
	await openProject(page, 'widgets');
	let compare = await openPr(page);
	const files = compare.getByRole('list', { name: 'Compared files' });
	await expect(compare.getByText('0/2 viewed')).toBeVisible();
	await files.getByRole('button', { name: 'gadget.ts', exact: true }).click();
	const diff = page.locator('.diff:not([hidden])');
	await expect(diff.locator('.path')).toHaveText('src/gadget.ts');

	await diff.getByRole('button', { name: 'Viewed & next' }).click();
	await expect(diff.locator('.path')).toHaveText('src/widget.ts');
	compare = await comparePanel(page);
	await expect(compare.getByText('1/2 viewed')).toBeVisible();
	const boxes = files.getByRole('checkbox', { name: 'Viewed' });
	await expect(boxes).toHaveCount(2);
	await expect(boxes.first()).toBeChecked();
	await expect(boxes.last()).not.toBeChecked();

	await boxes.first().uncheck();
	await expect(compare.getByText('0/2 viewed')).toBeVisible();
});

test('Draft comments start from the gutter or a tapped line number, show inline and listed, and survive a reload', async ({ page }, { project }) => {
	signIn('OPEN');
	const { push } = repository();
	push(['list.ts', LIST.replace('line15 = 15', 'line15 = 0')]);
	await openProject(page, 'widgets');
	const compare = await comparePanel(page);
	await compare.getByRole('button', { name: 'Pull request…' }).click();
	await page.getByRole('dialog', { name: 'Open a pull request' }).getByRole('option', { name: /Add widgets/ }).click();
	await compare.getByRole('list', { name: 'Compared files' }).getByRole('button', { name: 'list.ts', exact: true }).click();
	const diff = page.locator('.diff:not([hidden])');
	await expect(diff.locator('.cm-changedLine', { hasText: 'line15 = 0' })).toBeVisible();

	const adds = diff.locator('.cm-draftAdd');
	const desktop = project.name === 'desktop';
	await expect(adds).toHaveCount(desktop ? 14 : 8);
	const lines = await adds.evaluateAll((elements) => elements.map((element) => Number((element as HTMLElement).dataset.line)));
	expect(lines.every((line) => line >= 12 && line <= 18)).toBe(true);

	if (desktop) await diff.locator('.cm-draftAdd[data-side="source"][data-line="15"]').click();
	else await diff.locator('.cm-lineNumbers .cm-gutterElement', { hasText: /^15$/ }).click();
	const box = page.getByRole('textbox', { name: 'Draft comment on line 15' });
	await box.click();
	await page.keyboard.type('Why **zero**?');
	await diff.getByRole('button', { name: 'Save' }).click();
	const inline = diff.locator('.cm-draftComment[data-draft]');
	await expect(inline).toHaveCount(1);
	await expect(inline).toContainText('Why zero?');

	const listed = (await comparePanel(page)).getByRole('list', { name: 'Draft comments' });
	await expect(listed.getByRole('button')).toHaveCount(1);
	await expect(listed.getByRole('button')).toContainText('src/list.ts:15');
	await expect(listed.getByRole('button')).toContainText('Why **zero**?');

	await page.reload();
	await expect(inline).toContainText('Why zero?');
	const reloaded = (await comparePanel(page)).getByRole('list', { name: 'Draft comments' }).getByRole('button');
	await expect(reloaded).toContainText('src/list.ts:15');
	await reloaded.click();
	await expect(inline).toBeInViewport();
});

test('After a force-push and Refresh, a draft on a rewritten line is Displaced until Re-place puts it back inline', async ({ page }, { project }) => {
	signIn('OPEN');
	const { push } = repository();
	push(['list.ts', LIST.replace('line15 = 15', 'line15 = 0')]);
	await openProject(page, 'widgets');
	let compare = await openPr(page);
	await compare.getByRole('list', { name: 'Compared files' }).getByRole('button', { name: 'list.ts', exact: true }).click();
	const diff = page.locator('.diff:not([hidden])');
	const desktop = project.name === 'desktop';
	const pick = (line: number) =>
		desktop
			? diff.locator(`.cm-draftAdd[data-side="source"][data-line="${line}"]`).click()
			: diff.locator('.cm-lineNumbers .cm-gutterElement', { hasText: new RegExp(`^${line}$`) }).click();
	await pick(15);
	await page.getByRole('textbox', { name: 'Draft comment on line 15' }).click();
	await page.keyboard.type('Why zero?');
	await diff.getByRole('button', { name: 'Save' }).click();
	const inline = diff.locator('.cm-draftComment[data-draft]');
	await expect(inline).toHaveCount(1);

	push(['list.ts', LIST.replace('line15 = 15', 'line15 = -1')]);
	compare = await comparePanel(page);
	await compare.getByRole('button', { name: 'Refresh' }).click();
	const displaced = compare.getByRole('list', { name: 'Displaced' });
	await expect(displaced.getByRole('listitem')).toHaveCount(1);
	await expect(displaced).toContainText('src/list.ts:15');
	await expect(displaced).toContainText('Why zero?');
	await expect(compare.getByRole('button', { name: 'Send review…' })).toBeDisabled();
	await expect(compare).toContainText('Re-place or delete the Displaced draft to send.');
	await expect(compare.getByRole('list', { name: 'Draft comments' })).toHaveCount(0);
	await expect(inline).toHaveCount(0);

	await displaced.getByRole('button', { name: 'Re-place' }).click();
	await expect(diff.getByRole('status')).toContainText('Choose a line to re-place');
	await expect(diff.locator('.cm-draftCommentable').first()).toBeVisible();
	await pick(14);
	await expect(inline).toHaveCount(1);
	await expect(inline).toContainText('Why zero?');
	await expect(diff.getByRole('status')).toHaveCount(0);
	await expect(page.getByRole('textbox', { name: /^Draft comment on line/ })).toHaveCount(1);

	compare = await comparePanel(page);
	await expect(compare.getByRole('list', { name: 'Displaced' })).toHaveCount(0);
	await expect(compare.getByRole('button', { name: 'Send review…' })).toBeEnabled();
	await expect(compare.getByRole('list', { name: 'Draft comments' }).getByRole('button')).toContainText('src/list.ts:14');
});

function sentReviews(): { args: string[]; stdin: Record<string, unknown> }[] {
	const log = `${process.env.STAMPEDE_E2E_ROOT}/gh.log`;
	return existsSync(log) ? readFileSync(log, 'utf8').trim().split('\n').map((line) => JSON.parse(line)) : [];
}

test('Send review posts every draft as one review, keeps them when GitHub refuses, and keeps Viewed marks', async ({ page }, { project }) => {
	const { git, push } = repository();
	push(['list.ts', LIST.replace('line15 = 15', 'line15 = 0')]);
	const message = 'Validation Failed: Pull request review thread line must be part of the diff';
	signIn('OPEN', { fail: message });
	await openProject(page, 'widgets');
	let compare = await openPr(page);
	const files = compare.getByRole('list', { name: 'Compared files' });
	await files.getByRole('checkbox', { name: 'Viewed' }).check();
	await expect(compare.getByText('1/1 viewed')).toBeVisible();
	await files.getByRole('button', { name: 'list.ts', exact: true }).click();
	const diff = page.locator('.diff:not([hidden])');
	if (project.name === 'desktop') await diff.locator('.cm-draftAdd[data-side="source"][data-line="15"]').click();
	else await diff.locator('.cm-lineNumbers .cm-gutterElement', { hasText: /^15$/ }).click();
	await page.getByRole('textbox', { name: 'Draft comment on line 15' }).click();
	await page.keyboard.type('Why zero?');
	await diff.getByRole('button', { name: 'Save' }).click();
	await expect(diff.locator('.cm-draftComment[data-draft]')).toHaveCount(1);

	compare = await comparePanel(page);
	const drafts = compare.getByRole('list', { name: 'Draft comments' });
	await expect(drafts.getByRole('button')).toHaveCount(1);
	await compare.getByRole('button', { name: 'Send review… 1' }).click();
	const dialog = page.getByRole('dialog', { name: 'Send review of #7' });
	await expect(dialog.getByRole('list', { name: 'Draft comments to send' })).toContainText('src/list.ts:15');
	await expect(dialog.getByRole('list', { name: 'Draft comments to send' })).toContainText('Why zero?');
	await expect(dialog.getByRole('radio', { name: /^Comment/ })).toBeChecked();
	await dialog.getByRole('radio', { name: /^Request changes/ }).check();
	await dialog.getByRole('textbox', { name: 'Review summary' }).click();
	await page.keyboard.type('One question');
	await dialog.getByRole('button', { name: 'Send', exact: true }).click();
	await expect(dialog.getByRole('alert')).toHaveText(message);
	await expect(dialog.getByRole('textbox', { name: 'Review summary' })).toContainText('One question');
	await expect(drafts.getByRole('button')).toHaveCount(1);

	signIn('OPEN');
	await dialog.getByRole('button', { name: 'Send', exact: true }).click();
	await expect(dialog).toBeHidden();
	await expect(drafts).toHaveCount(0);
	await expect(compare.getByText('1/1 viewed')).toBeVisible();
	const sent = sentReviews();
	expect(sent).toHaveLength(2);
	expect(sent[1].args).toEqual(['api', '--method', 'POST', 'repos/acme/widgets/pulls/7/reviews', '--input', '-']);
	expect(sent[1].stdin).toEqual({
		commit_id: git('rev-parse', 'refs/stampede/pr/7/source'), body: 'One question', event: 'REQUEST_CHANGES',
		comments: [{ path: 'src/list.ts', body: 'Why zero?', side: 'RIGHT', line: 15 }]
	});
});

test('Own PRs offer only Comment, and Displaced drafts disable sending', async ({ page }) => {
	signIn('OPEN', { author: 'david' });
	repository();
	await openProject(page, 'widgets');
	const compare = await openPr(page);
	await compare.getByRole('button', { name: 'Send review…' }).click();
	const dialog = page.getByRole('dialog', { name: 'Send review of #7' });
	await expect(dialog.getByRole('radio')).toHaveCount(1);
	await expect(dialog.getByRole('radio', { name: /^Comment/ })).toBeChecked();
	await expect(dialog.getByRole('radio', { name: /^Approve/ })).toHaveCount(0);
	await expect(dialog.getByRole('radio', { name: /^Request changes/ })).toHaveCount(0);
	await dialog.getByRole('button', { name: 'Cancel' }).click();

	const reviews = `${process.env.STAMPEDE_E2E_ROOT}/data/pr-reviews`;
	const record = readdirSync(reviews).map((dir) => `${reviews}/${dir}/7.json`).find(existsSync)!;
	const review = JSON.parse(readFileSync(record, 'utf8'));
	const draft = { id: 'displaced', path: 'src/widget.ts', side: 'source', line: 1, body: 'Stranded', commit: review.source.commit, displaced: true };
	writeFileSync(record, JSON.stringify({ ...review, drafts: [draft] }));
	await page.reload();
	await expect(page.getByRole('list', { name: 'Displaced' })).toContainText('Stranded');
	const reloaded = await comparePanel(page);
	await expect(reloaded.getByRole('button', { name: 'Send review…' })).toBeDisabled();
	await expect(reloaded).toContainText('Re-place or delete the Displaced draft to send.');
});
