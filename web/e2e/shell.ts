import { expect, test } from './fixtures.ts';
import type { Page } from '@playwright/test';

function phone() {
	return test.info().project.name === 'pixel';
}

export async function openProject(page: Page, name: string) {
	await page.goto('/');
	if (phone()) {
		await page.getByRole('button', { name: 'Previous screen' }).click();
		await settled(page, 0);
	}
	await page.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name: 'Open Project' }).click();
	await page.getByRole('option', { name: new RegExp(`/${name}$`) }).click();
	await expect(page.getByRole('dialog')).toBeHidden();
	if (phone()) await settled(page, 1);
}

export async function showSidebar(page: Page) {
	const sidebar = page.getByRole('complementary', { name: 'Sidebar' });
	if (phone()) {
		await page.getByRole('button', { name: 'Next screen' }).click();
		await settled(page, 2);
	}
	else if (!(await sidebar.isVisible())) await page.getByRole('button', { name: 'Sidebar' }).click();
	return sidebar;
}

async function settled(page: Page, index: number) {
	await expect.poll(() => page.locator('.screens').evaluate((element, index) => {
		const screen = element.children[index] as HTMLElement;
		return !screen.inert && Math.abs(element.scrollLeft - element.clientWidth * index) < 1;
	}, index)).toBe(true);
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
