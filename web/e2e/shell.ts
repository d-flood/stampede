import { expect } from './fixtures.ts';
import type { Locator, Page } from '@playwright/test';

/** Whether the page shows the phone layout, which a phone resized to tablet width doesn't. */
async function phone(page: Page) {
	await page.locator('.phone, .shell').first().waitFor();
	return (await page.locator('.phone').count()) > 0;
}

/** Closes whichever phone sheet covers the Tab, as the system back gesture does. */
export async function closeSheet(page: Page) {
	const sheets = page.getByRole('region', { name: /^(Projects|Sidebar)$/ });
	if (!(await sheets.count())) return;
	await page.goBack();
	await expect(sheets).toHaveCount(0);
}

/** Opens the phone's Projects sheet; on desktop the Project rail is always shown. */
export async function showProjects(page: Page) {
	const projects = page.getByRole('navigation', { name: 'Projects' });
	if ((await phone(page)) && !(await projects.isVisible())) {
		await closeSheet(page);
		await page.locator('.phone > header').getByRole('button', { name: 'Projects', exact: true }).click();
	}
	await expect(projects).toBeVisible();
	return projects;
}

export async function openProject(page: Page, name: string) {
	await page.goto('/');
	await (await showProjects(page)).getByRole('button', { name: 'Open Project' }).click();
	await page.getByRole('option', { name: new RegExp(`/${name}$`) }).click();
	await expect(page.getByRole('dialog')).toBeHidden();
	if (await phone(page)) await expect(page.getByRole('navigation', { name: 'Projects' })).toBeHidden();
}

export async function showSidebar(page: Page) {
	const sidebar = page.getByRole('complementary', { name: 'Sidebar' });
	if (!(await sidebar.isVisible())) {
		await closeSheet(page);
		await page.getByRole('button', { name: 'Sidebar' }).click();
	}
	await expect(sidebar).toBeVisible();
	return sidebar;
}

/** Swipes with touch from a point sideways, or up and down when vertical, as a thumb would. */
export async function touchSwipe(page: Page, x: number, y: number, distance: number, vertical = false) {
	const session = await page.context().newCDPSession(page);
	try {
		await session.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x, y }] });
		for (let step = 1; step <= 5; step++) {
			const moved = (distance * step) / 5;
			await session.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [vertical ? { x, y: y + moved } : { x: x + moved, y }] });
		}
		await session.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
	} finally {
		await session.detach();
	}
}

/** Opens the phone New Tab sheet from the Tab strip. */
export async function openNewTab(page: Page) {
	await closeSheet(page);
	await page.getByRole('button', { name: 'New Tab', exact: true }).click();
	const sheet = page.getByRole('dialog', { name: 'New Tab', exact: true });
	await expect(sheet).toBeVisible();
	return sheet;
}

/** Starts a New Tab item from the desktop menu or the phone New Tab sheet. */
export async function newTab(page: Page, item: string) {
	if (await phone(page)) {
		await (await openNewTab(page)).getByRole('button', { name: item, exact: true }).click();
		await expect(page.getByRole('dialog', { name: 'New Tab', exact: true })).toBeHidden();
		return;
	}
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: item, exact: true }).click();
}

/** Follows a Tab by its id, since a Chat tab's name changes once its Chat has a title. */
export async function pinned(tab: Locator) {
	return tab.page().locator(`[data-tab="${await tab.getAttribute('data-tab')}"]`);
}

export async function chooseTab(page: Page, name: string | RegExp) {
	await page.getByRole('tab', { name }).click();
}

export async function openSettings(page: Page, section: 'General' | 'Appearance' | 'Agents' | 'Rigs' | 'Clips') {
	await showProjects(page);
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

/** Asserts whether a Tab is the one shown. */
export async function expectCurrent(page: Page, name: string, current = true) {
	await expect(page.getByRole('tab', { name }).first()).toHaveAttribute('aria-selected', String(current));
}

/** Closes the shown Tab from its close button. */
export async function closeTab(page: Page, name: string) {
	await page.getByRole('button', { name: `Close ${name}`, exact: true }).click();
}

export async function currentTab(page: Page) {
	return page.getByRole('tab', { selected: true });
}

export async function tabCount(page: Page) {
	return page.getByRole('tab').count();
}

export async function expectShown(page: Page, id: string, shown = true) {
	await expect(page.locator(`[data-tab="${id}"]`)).toHaveAttribute('aria-selected', String(shown));
}

export async function showTab(page: Page, id: string) {
	await page.locator(`[data-tab="${id}"]`).click();
}

export async function expectSelected(tab: Locator) {
	await expect(tab).toHaveAttribute('aria-selected', 'true');
}

/** Closes a pinned Tab from its context menu. */
export async function closePinned(tab: Locator) {
	await tab.click({ button: 'right' });
	await tab.page().getByRole('menuitem', { name: 'Close', exact: true }).click();
}
