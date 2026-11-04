import { expect, test } from './fixtures.ts';
import type { Page } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { showSidebar, openProject } from './shell.ts';

test.use({ reducedMotion: 'reduce' });

const URL = 'https://github.com/acme/widgets.git';

function signIn(state: 'OPEN' | 'MERGED') {
	const pr = {
		number: 7, title: 'Add widgets', author: { login: 'octo' }, isDraft: false, updatedAt: '2026-11-01T00:00:00Z', state,
		baseRefName: 'main', url: 'https://github.com/acme/widgets/pull/7', requested: true
	};
	const fixture = { user: 'david', repo: { nameWithOwner: 'acme/widgets', url: 'https://github.com/acme/widgets' }, prs: [pr] };
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
	git('add', '.');
	git('commit', '-qm', 'Initial commit');
	git('remote', 'add', 'origin', URL);
	git('config', `url.${origin}.insteadOf`, URL);
	git('push', '-q', 'origin', 'main');
	const push = (file: string) => {
		git('switch', '-q', '--detach', 'main');
		mkdirSync(`${root}/src`, { recursive: true });
		writeFileSync(`${root}/src/${file}`, `export const ${file.split('.')[0]} = true;\n`);
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
	await expect(page.getByRole('tab', { selected: true })).toContainText('#7');
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
