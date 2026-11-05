import { expect, test } from './fixtures.ts';
import type { Locator, Page } from '@playwright/test';
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

test('A Suggested draft holds the send until Accept makes it a plain Draft comment', async ({ page }) => {
	signIn('OPEN');
	repository();
	await openProject(page, 'widgets');
	await openPr(page);
	const reviews = `${process.env.STAMPEDE_E2E_ROOT}/data/pr-reviews`;
	const record = readdirSync(reviews).map((dir) => `${reviews}/${dir}/7.json`).find(existsSync)!;
	const review = JSON.parse(readFileSync(record, 'utf8'));
	const suggested = { chat: 'chat', rig: 'opus' };
	const draft = { id: 'suggested', path: 'src/widget.ts', side: 'source', line: 1, body: 'Name it widgets?', commit: review.source.commit, displaced: false, suggested };
	writeFileSync(record, JSON.stringify({ ...review, drafts: [draft] }));
	await page.reload();
	await expect(page.getByRole('list', { name: 'Suggested' })).toContainText('opus');
	let compare = await comparePanel(page);
	const listed = compare.getByRole('list', { name: 'Suggested' });
	await expect(listed).toContainText('Name it widgets?');
	await expect(compare.getByRole('button', { name: 'Send review…' })).toBeDisabled();
	await expect(compare).toContainText('Accept or delete the Suggested draft to send.');

	await listed.getByRole('button').click();
	const inline = page.locator('.diff:not([hidden]) .cm-draftComment[data-draft="suggested"]');
	await expect(inline).toContainText('Suggested by opus');
	await inline.getByRole('button', { name: 'Accept' }).click();
	await expect(inline.getByRole('button', { name: 'Accept' })).toHaveCount(0);
	await expect(inline).not.toContainText('Suggested by opus');

	compare = await comparePanel(page);
	await expect(compare.getByRole('list', { name: 'Suggested' })).toHaveCount(0);
	await expect(compare.getByRole('list', { name: 'Draft comments' })).toContainText('Name it widgets?');
	await expect(compare.getByRole('button', { name: 'Send review…' })).toBeEnabled();
});

const RANGES = LIST.replace('line5 = 5', 'line5 = 0').replace('line6 = 6', 'line6 = 0').replace('line25 = 25', 'line25 = 0');
const LONG = Array.from({ length: 300 }, (_, index) => `export const long${index + 1} = ${index + 1};\n`).join('');

async function rangesDiff(page: Page, file = 'list.ts') {
	signIn('OPEN');
	repository().push(['list.ts', RANGES], ['long.ts', LONG]);
	await openProject(page, 'widgets');
	const compare = await openPr(page);
	await compare.getByRole('list', { name: 'Compared files' }).getByRole('button', { name: file, exact: true }).click();
	const diff = page.locator('.diff:not([hidden])');
	await expect(diff.locator('.cm-draftAdd').first()).toBeVisible();
	return diff;
}

const add = (diff: Locator, line: number, side = 'source') => diff.locator(`.cm-draftAdd[data-side="${side}"][data-line="${line}"]`);
const lineNumber = (diff: Locator, line: number) => diff.locator('.cm-lineNumbers .cm-gutterElement', { hasText: new RegExp(`^${line}$`) }).last();
const commenting = (page: Page, where: string) => page.getByRole('textbox', { name: `Draft comment on ${where}`, exact: true });

async function press(page: Page, target: Locator) {
	const box = (await target.boundingBox())!;
	await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
	await page.mouse.down();
}

async function dragOver(page: Page, target: Locator, x = 0) {
	const box = (await target.boundingBox())!;
	await page.mouse.move(box.x + box.width / 2 + x, box.y + box.height / 2, { steps: 5 });
}

test('A gutter drag highlights its range, can end over the code, and sends as one multi-line comment', { tag: '@desktop' }, async ({ page }) => {
	const diff = await rangesDiff(page);
	await press(page, add(diff, 3));
	await dragOver(page, add(diff, 7), 120);
	await expect(diff.locator('.cm-draftSelected')).toHaveCount(5);
	await expect(diff.locator('.cm-draftAddOn')).toHaveCount(5);
	await page.mouse.up();

	const box = commenting(page, 'lines 3–7');
	await expect(box).toBeFocused();
	await expect(diff.locator('.cm-draftComment:not([data-draft])')).toContainText('Lines 3–7 · Shift-click another line to extend');
	await page.keyboard.type('Why zero?');
	await diff.getByRole('button', { name: 'Save' }).click();
	await expect(diff.locator('.cm-draftComment[data-draft]')).toContainText('Lines 3–7');
	await expect(diff.locator('.cm-draftRange')).toHaveCount(5);
	await expect(diff.locator('.cm-draftSelected')).toHaveCount(0);

	const compare = await comparePanel(page);
	await expect(compare.getByRole('list', { name: 'Draft comments' }).getByRole('button')).toContainText('src/list.ts:3–7');
	await compare.getByRole('button', { name: 'Send review… 1' }).click();
	const dialog = page.getByRole('dialog', { name: 'Send review of #7' });
	await dialog.getByRole('button', { name: 'Send', exact: true }).click();
	await expect(dialog).toBeHidden();
	expect(sentReviews()[0].stdin.comments).toEqual([{ path: 'src/list.ts', body: 'Why zero?', side: 'RIGHT', line: 7, start_line: 3, start_side: 'RIGHT' }]);
});

test('A drag stays within the hunk it started in, and line numbers drag like the gutter', { tag: '@desktop' }, async ({ page }) => {
	const diff = await rangesDiff(page);
	await press(page, add(diff, 3));
	await dragOver(page, add(diff, 24));
	await page.mouse.up();
	await expect(commenting(page, 'lines 3–9')).toBeVisible();
	await diff.getByRole('button', { name: 'Cancel' }).click();

	await press(page, lineNumber(diff, 23));
	await dragOver(page, lineNumber(diff, 26));
	await page.mouse.up();
	await expect(commenting(page, 'lines 23–26')).toBeVisible();
});

test('Shift-click extends the open draft either way, and every move keeps what was typed', { tag: '@desktop' }, async ({ page }) => {
	const diff = await rangesDiff(page);
	await add(diff, 5).click();
	await expect(commenting(page, 'line 5')).toBeFocused();
	await page.keyboard.type('half');
	await add(diff, 8).click({ modifiers: ['Shift'] });
	await expect(commenting(page, 'lines 5–8')).toBeFocused();
	await page.keyboard.type(' done');
	await add(diff, 3).click({ modifiers: ['Shift'] });
	await expect(commenting(page, 'lines 3–8')).toHaveText('half done');
	await add(diff, 25).click();
	await expect(commenting(page, 'line 25')).toHaveText('half done');
	await diff.getByRole('button', { name: 'Cancel' }).click();
	await add(diff, 25).click();
	await expect(commenting(page, 'line 25')).toHaveText('');
});

test('A long range survives scrolling: typed text outlasts the composer unmounting, and Shift-click reaches far lines', { tag: '@desktop' }, async ({ page }) => {
	const diff = await rangesDiff(page, 'long.ts');
	const scroller = diff.locator('.cm-mergeView');
	await add(diff, 5).click();
	await expect(commenting(page, 'line 5')).toBeFocused();
	await page.keyboard.type('half-written');
	await scroller.evaluate((element) => (element.scrollTop = element.scrollHeight));
	await expect(add(diff, 295)).toBeVisible();
	await expect(commenting(page, 'line 5')).toHaveCount(0);
	await scroller.evaluate((element) => (element.scrollTop = 0));
	await expect(commenting(page, 'line 5')).toHaveText('half-written');
	await expect(commenting(page, 'line 5')).not.toBeFocused();

	await scroller.evaluate((element) => (element.scrollTop = element.scrollHeight));
	await add(diff, 295).click({ modifiers: ['Shift'] });
	await expect(commenting(page, 'lines 5–295')).toHaveText('half-written');
});

test('Dragging to the edge of a long hunk autoscrolls in both layouts', { tag: '@desktop' }, async ({ page }) => {
	const diff = await rangesDiff(page, 'long.ts');
	for (const selector of ['.cm-mergeView', '.cm-scroller']) {
		const scroller = diff.locator(selector).first();
		const area = (await scroller.boundingBox())!;
		await press(page, add(diff, 5));
		await page.mouse.move(area.x + 200, area.y + area.height - 2, { steps: 4 });
		await expect.poll(() => scroller.evaluate((element) => element.scrollTop)).toBeGreaterThan(400);
		await page.mouse.up();
		const label = await diff.getByRole('textbox', { name: /^Draft comment on lines 5–\d+$/ }).getAttribute('aria-label');
		expect(Number(label!.split('–')[1])).toBeGreaterThan(30);
		await diff.getByRole('button', { name: 'Cancel' }).click();
		if (selector === '.cm-mergeView') await page.getByRole('button', { name: 'Single column' }).click();
	}
});

test('In one column a drag stays on the side it started on', { tag: '@desktop' }, async ({ page }) => {
	const diff = await rangesDiff(page);
	await page.getByRole('button', { name: 'Single column' }).click();
	await press(page, add(diff, 5, 'target'));
	await dragOver(page, add(diff, 8));
	await expect(diff.locator('.cm-draftAddOn')).toHaveCount(2);
	await page.mouse.up();
	await expect(commenting(page, 'deleted lines 5–6')).toBeVisible();
	await diff.getByRole('button', { name: 'Cancel' }).click();

	await press(page, add(diff, 4));
	await dragOver(page, add(diff, 7));
	await page.mouse.up();
	await expect(commenting(page, 'lines 4–7')).toBeVisible();
});

test('On a phone a tap extends the open draft within its hunk, and elsewhere moves it with its text', { tag: '@phone' }, async ({ page }) => {
	const diff = await rangesDiff(page);
	await lineNumber(diff, 3).tap();
	await expect(diff.locator('.cm-draftComment:not([data-draft])')).toContainText('Line 3 · Tap another line to extend');
	await commenting(page, 'line 3').click();
	await page.keyboard.type('half');
	await lineNumber(diff, 7).tap();
	await expect(commenting(page, 'lines 3–7')).toHaveText('half');
	await expect(diff.locator('.cm-draftSelected')).toHaveCount(5);
	await add(diff, 2).tap();
	await expect(commenting(page, 'lines 2–7')).toHaveText('half');
	await lineNumber(diff, 2).tap();
	await expect(commenting(page, 'line 2')).toHaveText('half');
	await lineNumber(diff, 24).tap();
	await expect(commenting(page, 'line 24')).toHaveText('half');
});

test('Switching layout keeps open comments, their text and range, and unsaved edits', { tag: '@desktop' }, async ({ page }) => {
	const diff = await rangesDiff(page);
	const [single, sideBySide] = [page.getByRole('button', { name: 'Single column' }), page.getByRole('button', { name: 'Side by side' })];

	await add(diff, 3).click();
	await add(diff, 7).click({ modifiers: ['Shift'] });
	await expect(commenting(page, 'lines 3–7')).toBeFocused();
	await page.keyboard.type('half');
	await single.click();
	await expect(commenting(page, 'lines 3–7')).toHaveText('half');
	await expect(commenting(page, 'lines 3–7')).toBeFocused();
	await expect(diff.locator('.cm-draftSelected')).toHaveCount(5);
	await sideBySide.click();
	await expect(commenting(page, 'lines 3–7')).toHaveText('half');
	await diff.getByRole('button', { name: 'Cancel' }).click();

	await add(diff, 5, 'target').click();
	await expect(commenting(page, 'deleted line 5')).toBeFocused();
	await page.keyboard.type('gone');
	await add(diff, 25).click();
	await expect(commenting(page, 'line 25')).toBeFocused();
	await single.click();
	await expect(commenting(page, 'deleted line 5')).toHaveText('gone');
	await expect(commenting(page, 'line 25')).toHaveCount(0);
	await sideBySide.click();
	await expect(commenting(page, 'deleted line 5')).toHaveText('gone');
	await diff.getByRole('button', { name: 'Cancel' }).click();

	await add(diff, 25).click();
	await expect(commenting(page, 'line 25')).toBeFocused();
	await page.keyboard.type('saved');
	await diff.getByRole('button', { name: 'Save' }).click();
	const saved = commenting(page, 'line 25');
	await expect(diff.locator('.cm-draftComment[data-draft]')).toHaveCount(1);
	await saved.click();
	await page.keyboard.press('End');
	await page.keyboard.type(' and edited');
	await single.click();
	await expect(saved).toHaveText('saved and edited');
	await expect(diff.getByRole('button', { name: 'Save' })).toBeVisible();
});

test('Switching layout scrolls a far-down open comment back into view', { tag: '@desktop' }, async ({ page }) => {
	const diff = await rangesDiff(page, 'long.ts');
	await diff.locator('.cm-mergeView').evaluate((element) => (element.scrollTop = element.scrollHeight));
	await add(diff, 295).click();
	await expect(commenting(page, 'line 295')).toBeFocused();
	await page.keyboard.type('far down');
	await page.getByRole('button', { name: 'Single column' }).click();
	await expect(commenting(page, 'line 295')).toHaveText('far down');
	await expect(commenting(page, 'line 295')).toBeInViewport();
});
