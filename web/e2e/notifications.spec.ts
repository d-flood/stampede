import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from './fixtures.ts';

test('notifications can be disabled, stay off after reload, and be enabled again', async ({ page, request }, { project }) => {
	const { key } = await (await request.get('/api/push/key')).json();
	const subscription = { endpoint: 'https://push.example.test/current', keys: { p256dh: key, auth: 'AAAAAAAAAAAAAAAAAAAAAA' } };
	await request.post('/api/push/subscribe', {
		data: { browserId: 'other-browser', subscription: { ...subscription, endpoint: 'https://push.example.test/other' } }
	});
	await page.addInitScript((subscription) => {
		Object.defineProperty(Notification, 'permission', { get: () => 'granted' });
		Notification.requestPermission = async () => 'granted';
		const active = () => localStorage.getItem('test-push-disabled') !== 'true';
		const current = {
			toJSON: () => subscription,
			unsubscribe: async () => { localStorage.setItem('test-push-disabled', 'true'); return true; }
		};
		const registration = { pushManager: {
			getSubscription: async () => active() ? current : null,
			subscribe: async () => { localStorage.removeItem('test-push-disabled'); return current; }
		} };
		Object.defineProperty(navigator.serviceWorker, 'getRegistration', { value: async () => registration });
		Object.defineProperty(navigator.serviceWorker, 'ready', { get: () => Promise.resolve(registration) });
	}, subscription);
	const saved = () => JSON.parse(readFileSync(join(process.env.STAMPEDE_E2E_ROOT!, 'data/push/subscriptions.json'), 'utf8'))
		.map((item: { browserId: string }) => item.browserId).sort();
	const control = (name: string) => page.getByRole(project.name === 'pixel' ? 'button' : 'menuitem', { name, exact: true });
	await page.goto('/');
	await expect.poll(() => page.evaluate(() => localStorage.getItem('browserId'))).not.toBeNull();
	const browserId = await page.evaluate(() => localStorage.getItem('browserId'));
	await expect.poll(saved).toEqual([browserId, 'other-browser'].sort());
	await page.getByRole('button', { name: 'Attention list', exact: true }).click();
	await control('Disable notifications').click();
	await expect.poll(saved).toEqual(['other-browser']);
	await expect.poll(() => page.evaluate(() => localStorage.getItem('test-push-disabled'))).toBe('true');
	await page.reload();
	await page.getByRole('button', { name: 'Attention list', exact: true }).click();
	await expect(control('Enable notifications')).toBeEnabled();
	expect(saved()).toEqual(['other-browser']);
	if (project.name === 'pixel') {
		const dialog = page.getByRole('dialog', { name: 'Needs attention' });
		await expect.poll(() => dialog.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
	}
	await control('Enable notifications').click();
	await expect.poll(saved).toEqual([browserId, 'other-browser'].sort());
	await page.reload();
	await page.getByRole('button', { name: 'Attention list', exact: true }).click();
	await expect(control('Disable notifications')).toBeEnabled();
	await page.route('**/api/push/subscribe', async (route) => {
		if (route.request().method() === 'DELETE') await route.fulfill({ status: 503 });
		else await route.continue();
	});
	await control('Disable notifications').click();
	if (project.name !== 'pixel') await page.getByRole('button', { name: 'Attention list', exact: true }).click();
	await expect(control('Retry disabling notifications')).toBeEnabled();
	await page.unroute('**/api/push/subscribe');
	await control('Retry disabling notifications').click();
	await expect.poll(saved).toEqual(['other-browser']);
});
