import { expect, test } from './fixtures.ts';
import type { Page } from '@playwright/test';

function phone() {
	return test.info().project.name === 'pixel';
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

export async function showScreen(page: Page, name: 'projects' | 'tabs' | 'sidebar') {
	const screens = page.locator('.screens');
	await expect(screens.locator('.screen')).toHaveCount(3);
	const names = await screens.locator('.screen').evaluateAll((elements) => elements.map((element) => (element as HTMLElement).dataset.screen!));
	const target = names.indexOf(name);
	const current = await screens.locator('.screen:not([inert])').getAttribute('data-screen');
	await settled(page, current!);
	const index = names.indexOf(current!);
	for (let step = index; step !== target; step += Math.sign(target - index)) {
		await page.getByRole('button', { name: target > index ? 'Next screen' : 'Previous screen' }).click();
		await settled(page, names[step + Math.sign(target - index)]);
	}
	await settled(page, name);
}

async function settled(page: Page, name: string) {
	await expect.poll(() => page.locator('.screens').evaluate((element, name) => {
		const screen = element.querySelector<HTMLElement>(`[data-screen="${name}"]`)!;
		return !screen.inert && Math.abs(element.scrollLeft - screen.offsetLeft) < 1 && element instanceof HTMLElement && !element.style.scrollSnapType;
	}, name)).toBe(true);
}

export async function openSettings(page: Page, section: 'General' | 'Appearance' | 'Agents' | 'Rigs' | 'Clips') {
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
