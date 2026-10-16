import { browserId } from '#lib/device.ts';

export type Notifications = 'unsupported' | 'blocked' | 'off' | 'on';

export async function notifications(): Promise<Notifications> {
	if (!('PushManager' in window)) return 'unsupported';
	if (Notification.permission === 'denied') return 'blocked';
	const registration = await navigator.serviceWorker.getRegistration();
	const subscription = await registration?.pushManager.getSubscription();
	if (Notification.permission !== 'granted' || !subscription) return 'off';
	await subscribe(subscription);
	return 'on';
}

export async function enable(): Promise<Notifications> {
	const permission = await Notification.requestPermission();
	if (permission !== 'granted') return permission === 'denied' ? 'blocked' : 'off';
	const registration = await navigator.serviceWorker.ready;
	const { key } = await (await fetch('/api/push/key')).json();
	const subscription = await registration.pushManager.subscribe({ userVisibleOnly: true, applicationServerKey: key });
	await subscribe(subscription);
	return 'on';
}

async function subscribe(subscription: PushSubscription) {
	await fetch('/api/push/subscribe', {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ browserId: browserId(), subscription })
	});
}
