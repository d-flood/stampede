import type { Item, ServerMessage } from '#lib/device.svelte.ts';
import { expect, test } from './fixtures.ts';
import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync, unlinkSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { closeProject, openProject } from './shell.ts';

function publish(path: string, title: string): Extract<Item, { kind: 'media' }> {
	const output = execFileSync(process.env.STAMPEDE_E2E_BINARY ?? fileURLToPath(new URL('../../target/debug/stampede', import.meta.url)), [
		'chat-tools', '--data-dir', `${process.env.STAMPEDE_E2E_ROOT}/data`
	], { input: JSON.stringify({ id: 1, method: 'tools/call', params: { name: 'render_media', arguments: { path, title } } }) + '\n' });
	const result = JSON.parse(output.toString()).result;
	expect(result.isError).toBe(false);
	unlinkSync(path);
	return { kind: 'media', ...result.structuredContent };
}

test('chat media survives source removal and reload, plays recordings, and fits the screen', async ({ page }, { project, repeatEachIndex }) => {
	const name = `media-${project.name}-${repeatEachIndex}`;
	const root = `${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`;
	mkdirSync(root, { recursive: true });
	copyFileSync(fileURLToPath(new URL('../static/icons/icon-512.png', import.meta.url)), `${root}/screenshot.png`);
	const recording = await page.evaluate(async () => {
		const canvas = document.createElement('canvas');
		canvas.width = 1280;
		canvas.height = 720;
		const context = canvas.getContext('2d')!;
		const stream = canvas.captureStream(10);
		const recorder = new MediaRecorder(stream, { mimeType: 'video/webm;codecs=vp8' });
		const chunks: Blob[] = [];
		recorder.ondataavailable = (event) => chunks.push(event.data);
		const stopped = new Promise<void>((resolve) => recorder.onstop = () => resolve());
		recorder.start();
		for (let frame = 0; frame < 5; frame++) {
			context.fillStyle = frame % 2 ? '#312e81' : '#4338ca';
			context.fillRect(0, 0, canvas.width, canvas.height);
			await new Promise((resolve) => setTimeout(resolve, 100));
		}
		recorder.stop();
		await stopped;
		stream.getTracks().forEach((track) => track.stop());
		return Array.from(new Uint8Array(await new Blob(chunks).arrayBuffer()));
	});
	writeFileSync(`${root}/recording.webm`, Buffer.from(recording));
	const items = [publish(`${root}/screenshot.png`, 'Application screenshot'), publish(`${root}/recording.webm`, 'Screen recording')];
	let chatId: string;
	await page.routeWebSocket('**/ws', (socket) => {
		const server = socket.connectToServer();
		server.onMessage((data) => {
			if (typeof data !== 'string') return socket.send(data);
			const message: ServerMessage = JSON.parse(data);
			if (message.type === 'chat.created') chatId = message.chat.id;
			if (message.type === 'chat.items' && message.chatId === chatId) socket.send(JSON.stringify({ ...message, items }));
			else socket.send(data);
		});
	});
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude', exact: true }).click();
	const image = page.getByRole('img', { name: 'Application screenshot' });
	const video = page.locator('video[aria-label="Screen recording"]');
	await expect(image).toBeVisible();
	await expect.poll(() => image.evaluate((node: HTMLImageElement) => node.complete && node.naturalWidth > 0)).toBe(true);
	await video.scrollIntoViewIfNeeded();
	await expect(video).toHaveAttribute('controls');
	await expect(video).toHaveAttribute('playsinline');
	await expect(video).not.toHaveAttribute('autoplay');
	await expect.poll(() => video.evaluate((node: HTMLVideoElement) => node.readyState)).toBeGreaterThanOrEqual(1);
	await video.evaluate((node: HTMLVideoElement) => node.play());
	await expect.poll(() => video.evaluate((node: HTMLVideoElement) => node.currentTime)).toBeGreaterThan(0);
	await video.evaluate((node: HTMLVideoElement) => { node.pause(); node.currentTime = 0.1; });
	await expect.poll(() => video.evaluate((node: HTMLVideoElement) => !node.seeking && Math.abs(node.currentTime - 0.1) < 0.05)).toBe(true);
	for (const media of [image, video]) {
		await expect.poll(() => media.evaluate((node) => node.getBoundingClientRect().width <= node.parentElement!.clientWidth)).toBe(true);
		await expect.poll(() => media.evaluate((node) => node.getBoundingClientRect().height <= innerHeight * 0.7 + 1)).toBe(true);
	}
	await expect(page.locator('.rail figure')).toHaveCount(0);
	await expect(page.getByRole('link', { name: 'Open Application screenshot' })).toHaveAttribute('href', `/api/media/${items[0].asset}`);
	const panel = page.getByRole('tabpanel').filter({ has: video });
	await expect.poll(() => panel.evaluate((node) => node.scrollWidth <= node.clientWidth)).toBe(true);
	await expect(page.getByText('Couldn’t load this', { exact: false })).toHaveCount(0);
	await page.reload();
	await expect(image).toBeVisible();
	await expect.poll(() => image.evaluate((node: HTMLImageElement) => node.naturalWidth)).toBeGreaterThan(0);
	await expect.poll(() => video.evaluate((node: HTMLVideoElement) => node.readyState)).toBeGreaterThanOrEqual(1);
	await closeProject(page, name);
});
