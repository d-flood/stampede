import { expect, test } from './fixtures.ts';
import type { Locator, Page } from '@playwright/test';

function phone() {
	return !!test.info().project.use.isMobile;
}

/** The phone layout, which shows the current Tab in its header instead of a tab strip. */
async function compact(page: Page) {
	await page.locator('.phone > header, .shell').first().waitFor();
	return (await page.locator('.phone > header').count()) > 0;
}

type Level = 'projects' | 'overview' | 'section' | 'tab';

/** The phone Tab's title: the current Tab's name and status mark. */
async function tabsHeading(page: Page) {
	await showLevel(page, 'tab');
	return page.locator('.phone > header h1');
}

/** Follows a Tab by its id, since a Chat tab's name changes once its Chat has a title. On phone, follows the current Tab in the header. */
export async function pinned(tab: Locator) {
	if (await compact(tab.page())) {
		const title = await tabsHeading(tab.page());
		return tab.page().locator(`.phone > header h1[data-current-tab="${await title.getAttribute('data-current-tab')}"]`);
	}
	return tab.page().locator(`[data-tab="${await tab.getAttribute('data-tab')}"]`);
}

export async function openProject(page: Page, name: string) {
	await page.goto('/');
	if (phone()) await showLevel(page, 'projects');
	await page.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name: 'Open Project' }).click();
	await page.getByRole('option', { name: new RegExp(`/${name}$`) }).click();
	await expect(page.getByRole('dialog')).toBeHidden();
	if (phone()) await expect(page.locator('[data-level="overview"]')).toBeVisible();
}

export async function showSidebar(page: Page) {
	const sidebar = page.getByRole('complementary', { name: 'Sidebar' });
	if (phone()) await showLevel(page, 'section');
	else if (!(await sidebar.isVisible())) await page.getByRole('button', { name: 'Sidebar' }).click();
	return sidebar;
}

async function shownLevel(page: Page) {
	await expect(page.locator('.levels')).toHaveAttribute('aria-busy', 'false');
	const level = page.locator('.levels > .level:not([hidden])');
	await expect(level).toHaveCount(1);
	return (await level.getAttribute('data-level')) as Level;
}

/** Navigates as a person would: the header's Projects and Project buttons going up, and a Project, Tab or Browse row going down. */
export async function showLevel(page: Page, name: Level) {
	const header = page.locator('.phone > header');
	const current = await shownLevel(page);
	if (current === name) return;
	if (name === 'projects') await header.getByRole('button', { name: 'Projects', exact: true }).click();
	else if (name === 'overview' && current === 'projects') await page.getByRole('navigation', { name: 'Projects' }).locator('[aria-current="true"]').first().click();
	else if (name === 'overview') await header.getByRole('button', { name: (await header.locator('h1 .project').textContent())!, exact: true }).click();
	else {
		await showLevel(page, 'overview');
		const overview = page.locator('[data-level="overview"]');
		if (name === 'tab') {
			const row = overview.getByRole('list', { name: 'Open Tabs' }).locator('[aria-current="true"]');
			const opened = page.locator('[data-level="tab"]');
			await expect.poll(async () => (await row.isVisible()) || (await opened.isVisible())).toBe(true);
			if (await row.isVisible()) await row.click();
		} else {
			const checked = page.locator('aside[aria-label="Sidebar"] [role="radio"][aria-checked="true"]');
			const section = (await checked.count()) ? (await checked.textContent())!.trim() : 'Files';
			await overview.getByRole('button', { name: new RegExp(`^${section}\\b`) }).click();
		}
	}
	await expect(page.locator(`[data-level="${name}"]`)).toBeVisible();
}

/** Swipes a phone row or Tab body sideways with touch, as a thumb would. */
export async function touchSwipe(page: Page, x: number, y: number, distance: number) {
	const session = await page.context().newCDPSession(page);
	try {
		await session.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x, y }] });
		for (let step = 1; step <= 5; step++) await session.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{ x: x + distance * step / 5, y }] });
		await session.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
	} finally {
		await session.detach();
	}
}

/** Opens the phone New Tab sheet from the Project overview. */
export async function openNewTab(page: Page) {
	await showLevel(page, 'overview');
	await page.locator('[data-level="overview"]').getByRole('button', { name: 'New Tab', exact: true }).click();
	const sheet = page.getByRole('dialog', { name: 'New Tab', exact: true });
	await expect(sheet).toBeVisible();
	return sheet;
}

/** The phone Project overview's row for an open Tab. */
async function overviewRow(page: Page, tab: Locator) {
	await showLevel(page, 'overview');
	return page.getByRole('list', { name: 'Open Tabs' }).getByRole('listitem').filter({ has: tab });
}

/** Swipes a phone overview row away, closing its Tab. */
export async function swipeClosed(row: Locator) {
	const box = (await row.boundingBox())!;
	await touchSwipe(row.page(), box.x + box.width - 20, box.y + box.height / 2, -box.width * 0.7);
}

/** Starts a New Tab item from the desktop menu, or on phone from the Launchpad of a Project without Tabs or else the New Tab sheet. */
export async function newTab(page: Page, item: string) {
	if (await compact(page)) {
		await showLevel(page, 'overview');
		const launchpad = page.locator('[data-level="overview"] .launchpad');
		await ((await launchpad.count()) ? launchpad : await openNewTab(page)).getByRole('button', { name: item, exact: true }).click();
		await expect(page.locator('[data-level="tab"]')).toBeVisible();
		return;
	}
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: item, exact: true }).click();
}

/** Shows an open Tab from the desktop strip or the phone Project overview. */
export async function chooseTab(page: Page, name: string | RegExp) {
	if (await compact(page)) {
		await showLevel(page, 'overview');
		await page.getByRole('list', { name: 'Open Tabs' }).getByRole('button', { name }).first().click();
		return;
	}
	await page.getByRole('tab', { name }).click();
}

export async function openSettings(page: Page, section: 'General' | 'Appearance' | 'Agents' | 'Rigs' | 'Clips') {
	if (await compact(page)) await showLevel(page, 'projects');
	await page.getByRole('button', { name: 'Settings', exact: true }).click();
	const settings = page.getByRole('dialog', { name: 'Settings', exact: true });
	await settings.getByRole('navigation', { name: 'Settings sections' }).getByRole('button', { name: new RegExp(`^${section}`) }).click();
	return settings;
}

export function screen(page: Page) {
	return page.evaluate(() => {
		const buffer = window.terminal?.buffer.active;
		if (!buffer) return '';
		const lines = Array.from({ length: buffer.length }, (_, y) => buffer.getLine(y));
		return lines.map((line) => line?.translateToString(true) ?? '').join('\n');
	});
}

export async function closeProject(page: Page, name: string) {
	await page.evaluate((path) => new Promise<void>((resolve, reject) => {
		const socket = new WebSocket(`${location.protocol === 'https:' ? 'wss:' : 'ws:'}//${location.host}/ws`);
		let closing = false;
		const timer = setTimeout(() => { socket.close(); reject(new Error('Project cleanup timed out')); }, 5000);
		socket.onopen = () => socket.send(JSON.stringify({ type: 'projects.list' }));
		socket.onmessage = ({ data }) => {
			if (typeof data !== 'string') return;
			const message = JSON.parse(data);
			if (message.type !== 'projects.changed') return;
			const project = message.projects.find((project: { path: string }) => project.path === path);
			if (!project) { clearTimeout(timer); socket.close(); resolve(); }
			else if (!closing) { closing = true; socket.send(JSON.stringify({ type: 'project.close', projectId: project.id })); }
		};
	}), `${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`);
}

/** Asserts whether a Tab is the one shown: selected in the desktop strip, or titling the phone header. */
export async function expectCurrent(page: Page, name: string, current = true) {
	if (await compact(page)) {
		if (!current && !(await tabCount(page))) return;
		const title = (await tabsHeading(page)).locator('.name');
		if (current) await expect(title).toHaveText(name);
		else await expect(title).not.toHaveText(name);
		return;
	}
	await expect(page.getByRole('tab', { name }).first()).toHaveAttribute('aria-selected', String(current));
}

/** Closes a Tab from its close button, or on phone by swiping its Project overview row away. */
export async function closeTab(page: Page, name: string) {
	if (await compact(page)) return swipeClosed(await overviewRow(page, page.getByRole('button', { name, exact: true })));
	await page.getByRole('button', { name: `Close ${name}`, exact: true }).click();
}

/** The shown Tab: selected in the desktop strip, or the phone header's title. */
export async function currentTab(page: Page) {
	return (await compact(page)) ? tabsHeading(page) : page.getByRole('tab', { selected: true });
}

/** Counts open Tabs from the desktop strip, the phone header's position on a Tab, or else the phone Project overview before returning to where it was. */
export async function tabCount(page: Page) {
	if (!(await compact(page))) return page.getByRole('tab').count();
	const was = await shownLevel(page);
	if (was === 'tab') {
		const position = page.locator('.phone > header h1 .position');
		return (await position.count()) ? Number((await position.textContent())?.split('/')[1]) : 1;
	}
	await showLevel(page, 'overview');
	const count = await page.getByRole('list', { name: 'Open Tabs' }).getByRole('listitem').count();
	if (was !== 'overview') await showLevel(page, was);
	return count;
}

/** Asserts whether the Tab with this id is shown, from the desktop strip or the phone header. */
export async function expectShown(page: Page, id: string, shown = true) {
	if (!(await compact(page))) {
		await expect(page.locator(`[data-tab="${id}"]`)).toHaveAttribute('aria-selected', String(shown));
		return;
	}
	const title = await tabsHeading(page);
	if (shown) await expect(title).toHaveAttribute('data-current-tab', id);
	else await expect(title).not.toHaveAttribute('data-current-tab', id);
}

/** Shows the Tab with this id from the desktop strip or the phone Project overview. */
export async function showTab(page: Page, id: string) {
	if (await compact(page)) await showLevel(page, 'overview');
	await page.locator(`[data-tab="${id}"]`).click();
}

/** Asserts a pinned Tab is shown; on phone, pinned resolves to the header, which always shows the current Tab. */
export async function expectSelected(tab: Locator) {
	if (await compact(tab.page())) await expect(tab).toBeVisible();
	else await expect(tab).toHaveAttribute('aria-selected', 'true');
}

/** Closes a pinned Tab from its context menu, or on phone by swiping its Project overview row away. */
export async function closePinned(tab: Locator) {
	const page = tab.page();
	if (!(await compact(page))) {
		await tab.click({ button: 'right' });
		await page.getByRole('menuitem', { name: 'Close', exact: true }).click();
		return;
	}
	await showLevel(page, 'tab');
	const id = await tab.getAttribute('data-current-tab');
	await swipeClosed(await overviewRow(page, page.locator(`[data-tab="${id}"]`)));
}
