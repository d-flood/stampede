import { expect, test } from '@playwright/test';

test('app loads and shows the version', async ({ page, request }) => {
	const { version } = await (await request.get('/api/version')).json();
	await page.goto('/');
	await expect(page.getByText(`Version ${version}`)).toBeVisible();
});
