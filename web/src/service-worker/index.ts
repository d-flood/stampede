import { self } from '$app/service-worker';

type Item = { projectId: string; chatId: string; label: string; status: 'waiting' | 'done' };

self.addEventListener('install', () => self.skipWaiting());

self.addEventListener('push', (event) => {
	const item: Item = event.data?.json();
	event.waitUntil(self.registration.showNotification(`${item.label} — ${item.status}`, { data: item }));
});

self.addEventListener('notificationclick', (event) => {
	event.notification.close();
	event.waitUntil(open(event.notification.data));
});

async function open(item: Item) {
	const [client] = await self.clients.matchAll({ type: 'window', includeUncontrolled: true });
	if (!client) {
		const query = new URLSearchParams({ project: item.projectId, tab: item.chatId });
		await self.clients.openWindow(`/?${query}`);
		return;
	}
	await client.focus();
	client.postMessage(item);
}
