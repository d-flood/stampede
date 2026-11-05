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

/** The phone Tabs screen's title: the current Tab's name and status mark. */
async function tabsHeading(page: Page) {
	await showScreen(page, 'tabs');
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
	if (phone()) {
		await showScreen(page, 'projects');
	}
	await page.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name: 'Open Project' }).click();
	await page.getByRole('option', { name: new RegExp(`/${name}$`) }).click();
	await expect(page.getByRole('dialog')).toBeHidden();
	if (phone()) await settled(page, 'tabs');
}

export async function showSidebar(page: Page) {
	const sidebar = page.getByRole('complementary', { name: 'Sidebar' });
	if (phone()) {
		await showScreen(page, 'sidebar');
	}
	else if (!(await sidebar.isVisible())) await page.getByRole('button', { name: 'Sidebar' }).click();
	return sidebar;
}

/** Navigates as a person would: the header's left button where it leads, a focused Project for Tabs, and a swipe otherwise. */
export async function showScreen(page: Page, name: 'projects' | 'tabs' | 'sidebar') {
	const screens = page.locator('.screens');
	await expect(screens.locator('.screen')).toHaveCount(3);
	const names = await screens.locator('.screen').evaluateAll((elements) => elements.map((element) => (element as HTMLElement).dataset.screen!));
	const active = async () => (await screens.locator('.screen:not([inert])').getAttribute('data-screen'))!;
	let current = await active();
	await settled(page, current);
	for (let step = 0; current !== name; step++) {
		expect(step, `reach the ${name} screen`).toBeLessThan(3);
		const index = names.indexOf(current);
		const back = current === 'tabs' ? 'projects' : names[index - 1] ?? names[index + 1];
		const focused = page.getByRole('navigation', { name: 'Projects' }).locator('[aria-current="true"]').first();
		if (back !== name && name === 'tabs' && current === 'projects' && (await focused.count())) await focused.click();
		else if (back !== name && current === 'tabs') await swipe(page, names[index + 1] === name ? -1 : 1);
		else await page.locator('.phone > header > button').first().click();
		const previous = current;
		await expect.poll(active).not.toBe(previous);
		current = await active();
		await settled(page, current);
	}
}

async function swipe(page: Page, direction: -1 | 1) {
	const box = (await page.locator('.phone > header').first().boundingBox())!;
	const x = direction === -1 ? page.viewportSize()!.width - 20 : 20;
	await touchSwipe(page, x, box.y + box.height + 8, direction * 200);
}

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

/** Opens the phone Tab switcher from the header title. */
export async function openSwitcher(page: Page) {
	await (await tabsHeading(page)).getByRole('button').click();
	const switcher = page.getByRole('dialog', { name: 'Tabs', exact: true });
	await expect(switcher).toBeVisible();
	return switcher;
}

/** Starts a New Tab item from the desktop menu or the phone Tab switcher. */
export async function newTab(page: Page, item: string) {
	if (await compact(page)) {
		await (await openSwitcher(page)).getByRole('button', { name: item, exact: true }).click();
		return;
	}
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: item, exact: true }).click();
}

/** Shows an open Tab from the desktop strip or the phone Tab switcher. */
export async function chooseTab(page: Page, name: string | RegExp) {
	if (await compact(page)) {
		await (await openSwitcher(page)).getByRole('list', { name: 'Open Tabs' }).getByRole('button', { name }).first().click();
		return;
	}
	await page.getByRole('tab', { name }).click();
}

async function settled(page: Page, name: string) {
	await expect.poll(() => page.locator('.screens').evaluate((element, name) => {
		const screen = element.querySelector<HTMLElement>(`[data-screen="${name}"]`)!;
		return !screen.inert && Math.abs(element.scrollLeft - screen.offsetLeft) < 1 && element instanceof HTMLElement && !element.style.scrollSnapType;
	}, name)).toBe(true);
}

export async function openSettings(page: Page, section: 'General' | 'Appearance' | 'Agents' | 'Rigs' | 'Clips') {
	if (await compact(page)) await showScreen(page, 'projects');
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
		const title = (await tabsHeading(page)).locator('.name');
		if (current) await expect(title).toHaveText(name);
		else await expect(title).not.toHaveText(name);
		return;
	}
	await expect(page.getByRole('tab', { name }).first()).toHaveAttribute('aria-selected', String(current));
}

/** Closes a Tab from its close button, which on phone sits in the Tab switcher. */
export async function closeTab(page: Page, name: string) {
	const scope = (await compact(page)) ? await openSwitcher(page) : page;
	await scope.getByRole('button', { name: `Close ${name}`, exact: true }).click();
}

/** The shown Tab: selected in the desktop strip, or the phone header's title. */
export async function currentTab(page: Page) {
	return (await compact(page)) ? tabsHeading(page) : page.getByRole('tab', { selected: true });
}

/** Counts open Tabs from the desktop strip, or from the phone header's position, which a lone Tab doesn't show. */
export async function tabCount(page: Page) {
	if (!(await compact(page))) return page.getByRole('tab').count();
	const title = await tabsHeading(page);
	const position = title.locator('.position');
	if (await position.count()) return Number((await position.textContent())?.split('/')[1]);
	return (await title.getAttribute('data-current-tab')) ? 1 : 0;
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

/** Shows the Tab with this id from the desktop strip or the phone Tab switcher. */
export async function showTab(page: Page, id: string) {
	const scope = (await compact(page)) ? await openSwitcher(page) : page;
	await scope.locator(`[data-tab="${id}"]`).click();
}

/** Asserts a pinned Tab is shown; on phone, pinned resolves to the header, which always shows the current Tab. */
export async function expectSelected(tab: Locator) {
	if (await compact(tab.page())) await expect(tab).toBeVisible();
	else await expect(tab).toHaveAttribute('aria-selected', 'true');
}

/** Closes a pinned Tab from its context menu, or on phone from the Tab switcher. */
export async function closePinned(tab: Locator) {
	const page = tab.page();
	if (!(await compact(page))) {
		await tab.click({ button: 'right' });
		await page.getByRole('menuitem', { name: 'Close', exact: true }).click();
		return;
	}
	const id = await tab.getAttribute('data-current-tab');
	await (await openSwitcher(page)).getByRole('listitem').filter({ has: page.locator(`[data-tab="${id}"]`) }).getByRole('button', { name: /^Close / }).click();
}
