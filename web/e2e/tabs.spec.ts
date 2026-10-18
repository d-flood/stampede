import { expect, test } from './fixtures.ts';
import type { Page } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { closeProject, openProject, showSidebar } from './shell.ts';

async function chooseNewTab(page: Page, label: string) {
	await page.keyboard.press('Control+T');
	const picker = page.getByRole('dialog', { name: 'New Tab', exact: true });
	const query = picker.getByRole('combobox');
	await expect(query).toBeFocused();
	await query.fill(label);
	await page.keyboard.press('Enter');
	await expect(picker).toBeHidden();
}

test('active tabs expose a close button and preserve unsaved edits', async ({ page }, { project }) => {
	await openProject(page, 'alpha');
	const sidebar = await showSidebar(page);
	const file = `${project.name}.txt`;
	await sidebar.getByRole('button', { name: file, exact: true }).click();
	if (project.name === 'pixel') {
		await expect.poll(() => page.locator('.screens').evaluate((element) =>
			Math.abs(element.scrollLeft - element.clientWidth) < 1
		)).toBe(true);
	}
	const tabs = page.getByRole('tablist', { name: 'Tabs', exact: true });
	const close = tabs.getByRole('button', { name: `Close ${file}`, exact: true });
	await expect(close).toBeVisible();
	await expect.poll(async () => {
		const box = await close.boundingBox();
		return box && box.width >= (project.name === 'pixel' ? 48 : 26) && box.height >= (project.name === 'pixel' ? 48 : 26);
	}).toBe(true);
	await page.screenshot({ path: test.info().outputPath('close-file-tab.png') });
	await page.locator('.editor:not([hidden]) .cm-content').click();
	await page.keyboard.type('unsaved');
	if (project.name === 'pixel') await close.tap();
	else await close.click();
	const dialog = page.getByRole('dialog', { name: 'Discard unsaved changes?' });
	await expect(dialog).toBeVisible();
	await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
	await expect(tabs.getByRole('tab', { name: file })).toHaveAttribute('aria-selected', 'true');
	await page.keyboard.press('Control+W');
	await dialog.getByRole('button', { name: 'Discard', exact: true }).click();
	await expect(tabs.getByRole('tab', { name: file })).toHaveCount(0);
});

test('terminal and drawer tabs close directly on desktop and tablet', async ({ page }, { project }) => {
	if (project.name === 'pixel') await page.setViewportSize({ width: 1024, height: 768 });
	await page.goto('/');
	await page.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name: 'Open Project' }).click();
	await page.getByRole('option', { name: /\/beta$/ }).click();
	await expect(page.getByRole('dialog')).toBeHidden();
	const tabs = page.getByRole('tablist', { name: 'Tabs', exact: true });
	for (let i = 0; i < 2; i++) {
		await page.getByRole('button', { name: 'New Tab', exact: true }).click();
		await page.getByRole('menuitem', { name: 'Terminal', exact: true }).click();
		await expect(tabs.getByRole('tab')).toHaveCount(i + 1);
		await expect(tabs.getByRole('tab').last()).toHaveAttribute('aria-selected', 'true');
	}
	await expect(tabs.getByRole('button', { name: /^Close / })).toHaveCount(1);
	const close = tabs.getByRole('button', { name: /^Close / });
	await expect.poll(async () => {
		const box = await close.boundingBox();
		return box && box.width >= (project.name === 'pixel' ? 48 : 26) && box.height >= (project.name === 'pixel' ? 48 : 26);
	}).toBe(true);
	await page.screenshot({ path: test.info().outputPath('close-terminal-tab.png') });
	if (project.name === 'pixel') await close.tap();
	else await close.click();
	await expect(tabs.getByRole('tab')).toHaveCount(1);
	await expect(tabs.getByRole('tab')).toHaveAttribute('aria-selected', 'true');
	await close.click();
	await expect(tabs.getByRole('tab')).toHaveCount(0);
	await page.keyboard.press('Control+Backquote');
	const drawer = page.getByRole('region', { name: 'Drawer' });
	await expect.poll(() => drawer.evaluate((element) => getComputedStyle(element).transform)).toBe('none');
	await drawer.getByRole('button', { name: 'New Drawer Tab' }).click();
	await page.getByRole('menuitem', { name: 'Terminal', exact: true }).click();
	await expect(drawer.getByRole('tab')).toHaveCount(1);
	const closeDrawerTab = drawer.getByRole('button', { name: /^Close / });
	if (project.name === 'pixel') await closeDrawerTab.tap();
	else await closeDrawerTab.click();
	await expect(drawer.getByRole('tab')).toHaveCount(0);
});

test('tab shortcuts create, activate, and close tabs in the focused area', async ({ page }, { project }) => {
	await openProject(page, project.name === 'desktop' ? 'alpha' : 'beta');
	const tabs = page.getByRole('tablist', { name: 'Tabs', exact: true }).getByRole('tab');
	const count = await tabs.count();
	for (let i = 1; i <= 2; i++) {
		await chooseNewTab(page, 'Terminal');
		await expect(tabs).toHaveCount(count + i);
		await expect(tabs.last()).toHaveAttribute('aria-selected', 'true');
		await expect(page.locator('.terminal:visible [contenteditable="true"]')).toBeFocused();
	}
	const first = await tabs.nth(count).getAttribute('data-tab');
	await page.keyboard.press('Control+W');
	await expect(tabs).toHaveCount(count + 1);
	await expect(page.locator(`[data-tab="${first}"]`)).toHaveAttribute('aria-selected', 'true');
	await expect(page.locator('.terminal:visible [contenteditable="true"]')).toBeFocused();
	await page.keyboard.press('Control+Backquote');
	const drawer = page.getByRole('region', { name: 'Drawer' });
	await expect.poll(() => drawer.evaluate((element) => getComputedStyle(element).transform)).toBe('none');
	for (let i = 1; i <= 2; i++) {
		await chooseNewTab(page, 'Terminal');
		await expect(drawer.getByRole('tab')).toHaveCount(i);
		await expect(drawer.getByRole('tab').last()).toHaveAttribute('aria-selected', 'true');
		await expect(drawer.locator('.terminal:visible [contenteditable="true"]')).toBeFocused();
	}
	for (let i = 1; i >= 0; i--) {
		await page.keyboard.press('Control+W');
		await expect(drawer.getByRole('tab')).toHaveCount(i);
	}
	await page.keyboard.press('Control+W');
	await expect(tabs).toHaveCount(count + 1);
	await page.keyboard.press('Control+Backquote');
	await expect(drawer).toHaveAttribute('inert', '');
	await page.keyboard.press('Control+W');
	await expect(tabs).toHaveCount(count);
});

test('opening a tab from the menu keeps the new tab active', async ({ page }, { project }) => {
	await openProject(page, project.name === 'desktop' ? 'alpha' : 'beta');
	const tabs = page.getByRole('tablist', { name: 'Tabs', exact: true });
	await page.getByRole('button', { name: 'New Tab', exact: true }).click();
	await page.getByRole('menuitem', { name: 'Terminal', exact: true }).click();
	await expect(page.locator('.terminal:visible canvas')).toBeVisible();
	await page.getByRole('button', { name: 'New Tab', exact: true }).click();
	await page.getByRole('menuitem', { name: 'Browser', exact: true }).click();
	const browser = tabs.getByRole('tab', { name: 'Browser', exact: true });
	await expect(browser).toHaveAttribute('aria-selected', 'true');
	await expect(page.getByRole('menu')).toBeHidden();
	await expect(browser).toHaveAttribute('aria-selected', 'true');
});

test('created tabs activate before the tab list broadcast arrives', async ({ page }, { project }) => {
	let delaying = false;
	let created = '';
	const held: (() => void)[] = [];
	await page.routeWebSocket('**/ws', (socket) => {
		const server = socket.connectToServer();
		server.onMessage((message) => {
			if (typeof message === 'string') {
				const data = JSON.parse(message);
				if (data.type === 'terminal.created') created = data.terminal.id;
				if (data.type === 'chat.created') created = data.chat.id;
				if (delaying && ['terminals.changed', 'chats.changed'].includes(data.type)) {
					held.push(() => socket.send(message));
					return;
				}
			}
			socket.send(message);
		});
	});
	const folder = `tab-race-${project.name}`;
	mkdirSync(`${process.env.OIKONOMOS_E2E_ROOT}/projects/${folder}`, { recursive: true });
	await openProject(page, folder);
	for (const label of ['Terminal', 'Fake Claude']) {
		delaying = true;
		created = '';
		await page.getByRole('button', { name: 'New Tab', exact: true }).click();
		await page.getByRole('menuitem', { name: label, exact: true }).click();
		await expect.poll(() => created).not.toBe('');
		const tab = page.locator(`[data-tab="${created}"]`);
		await expect(tab).toHaveAttribute('aria-selected', 'true');
		delaying = false;
		for (const release of held.splice(0)) release();
		await expect(tab).toHaveAttribute('aria-selected', 'true');
	}
	await closeProject(page, folder);
});

test('new tab picker matches the menu and focuses the selected content', async ({ page, context }, { project }) => {
	await context.grantPermissions(['clipboard-read', 'clipboard-write']);
	const folder = `tab-picker-${project.name}`;
	mkdirSync(`${process.env.OIKONOMOS_E2E_ROOT}/projects/${folder}`, { recursive: true });
	await openProject(page, folder);
	await page.getByRole('button', { name: 'New Tab', exact: true }).click();
	await page.getByRole('menuitem', { name: 'Terminal', exact: true }).click();
	const terminal = page.locator('.terminal:visible [contenteditable="true"]');
	await expect(terminal).toBeVisible();
	await page.getByRole('button', { name: 'New Tab', exact: true }).click();
	const labels = await page.getByRole('menu').locator('.label').allTextContents();
	await page.keyboard.press('Escape');
	await terminal.focus();
	const tabs = page.getByRole('tablist', { name: 'Tabs', exact: true }).getByRole('tab');
	const count = await tabs.count();
	await page.keyboard.press('Control+T');
	const picker = page.getByRole('dialog', { name: 'New Tab', exact: true });
	await expect(picker.getByRole('option')).toHaveCount(labels.length);
	expect(await picker.locator('.item .label').allTextContents()).toEqual(labels);
	await page.keyboard.press('Control+W');
	await expect(picker).toBeVisible();
	await expect(tabs).toHaveCount(count);
	await page.keyboard.press('Escape');
	await expect(picker).toBeHidden();
	await expect(terminal).toBeFocused();
	await chooseNewTab(page, 'Browser');
	await expect(tabs.last()).toHaveAttribute('aria-selected', 'true');
	const address = page.getByRole('textbox', { name: 'Host address', exact: true });
	await expect(address).toBeFocused();
	await address.fill('normal copy');
	await page.keyboard.press('Control+A');
	await page.keyboard.press('Control+C');
	await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe('normal copy');
	await page.keyboard.press('Control+W');
	await expect(tabs).toHaveCount(count);
	await expect(terminal).toBeFocused();
	await chooseNewTab(page, 'Fake Claude');
	const chat = page.getByRole('tab', { name: /^Fake Claude$/ });
	await expect(chat).toHaveAttribute('aria-selected', 'true');
	await expect(page.getByRole('textbox', { name: 'Composer', exact: true })).toBeFocused();
	await expect(chat.getByTitle('done')).toBeVisible();
	await page.keyboard.press('Control+W');
	await expect(chat).toHaveCount(0);
	await chooseNewTab(page, 'Open file');
	const files = page.getByRole('dialog', { name: 'Open File', exact: true });
	await expect(files.getByRole('combobox')).toBeFocused();
	await expect(files.getByRole('combobox')).toHaveValue('');
	await page.keyboard.press('Escape');
	await closeProject(page, folder);
});
