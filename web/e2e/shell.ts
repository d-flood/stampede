import { expect, test, type Page } from '@playwright/test';

function phone() {
	return test.info().project.name === 'pixel';
}

export async function openProject(page: Page, name: string) {
	await page.goto('/');
	if (phone()) await page.getByRole('button', { name: 'Previous screen' }).click();
	await page.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name: 'Open Project' }).click();
	await page.getByRole('option', { name: new RegExp(`/${name}$`) }).click();
	await expect(page.getByRole('dialog')).toBeHidden();
}

export async function showSidebar(page: Page) {
	const sidebar = page.getByRole('complementary', { name: 'Sidebar' });
	if (phone()) await page.getByRole('button', { name: 'Next screen' }).click();
	else if (!(await sidebar.isVisible())) await page.getByRole('button', { name: 'Sidebar' }).click();
	return sidebar;
}

export function screen(page: Page) {
	return page.evaluate(() => {
		const buffer = window.terminal?.buffer.active;
		if (!buffer) return '';
		const lines = Array.from({ length: buffer.length }, (_, y) => buffer.getLine(y));
		return lines.map((line) => line?.translateToString(true) ?? '').join('\n');
	});
}
