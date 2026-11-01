import type { Locator, Page } from '@playwright/test';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

export const footage = fileURLToPath(new URL('../../promo/public/footage/', import.meta.url));

type Event = { t: number; device: string; type: string; x?: number; y?: number; name?: string; box?: { x: number; y: number; width: number; height: number } };

const now = () => Date.now() / 1000;
const wait = (seconds: number) => new Promise((resolve) => setTimeout(resolve, seconds * 1000));

export class Director {
	events: Event[] = [];
	private stops: (() => Promise<void>)[] = [];

	constructor(private scene: string) {
		rmSync(join(footage, scene), { recursive: true, force: true });
		mkdirSync(join(footage, scene), { recursive: true });
	}

	async record(device: string, page: Page) {
		const dir = join(footage, this.scene, device);
		mkdirSync(dir, { recursive: true });
		const viewport = page.viewportSize()!;
		const scale = await page.evaluate(() => devicePixelRatio);
		const cdp = await page.context().newCDPSession(page);
		const frames: { t: number; file: string }[] = [];
		const writes: Promise<void>[] = [];
		cdp.on('Page.screencastFrame', ({ data, metadata, sessionId }) => {
			const file = `${String(frames.length).padStart(5, '0')}.jpg`;
			frames.push({ t: metadata.timestamp!, file });
			writes.push(writeFile(join(dir, file), Buffer.from(data, 'base64')));
			cdp.send('Page.screencastFrameAck', { sessionId }).catch(() => {});
		});
		await cdp.send('Page.startScreencast', { format: 'jpeg', quality: 92, maxWidth: viewport.width * scale, maxHeight: viewport.height * scale });
		this.stops.push(async () => {
			await cdp.send('Page.stopScreencast');
			await Promise.all(writes);
			writeFileSync(join(dir, 'frames.json'), JSON.stringify({ viewport, scale, frames }));
		});
	}

	mark(device: string, name: string) {
		this.events.push({ t: now(), device, type: 'mark', name });
	}

	keys(device: string, name: string) {
		this.events.push({ t: now(), device, type: 'keys', name });
	}

	async hold(seconds: number) {
		await wait(seconds);
	}

	async point(device: string, target: Locator, travel = 0.7) {
		const box = (await target.boundingBox())!;
		this.events.push({ t: now(), device, type: 'move', x: box.x + box.width / 2, y: box.y + box.height / 2, box });
		await wait(travel);
		return box;
	}

	async rest(device: string, page: Page, x: number, y: number) {
		this.events.push({ t: now(), device, type: 'move', x, y });
		await page.mouse.move(x, y, { steps: 12 });
	}

	async click(device: string, target: Locator, travel = 0.7) {
		await this.point(device, target, travel * 0.7);
		await target.hover();
		await wait(travel * 0.3);
		this.events.push({ t: now(), device, type: 'click' });
		await target.click();
	}

	async tap(device: string, target: Locator, travel?: number) {
		const box = await this.point(device, target, travel);
		this.events.push({ t: now(), device, type: 'tap', x: box.x + box.width / 2, y: box.y + box.height / 2 });
		await target.tap();
	}

	async type(device: string, target: Locator, text: string) {
		this.events.push({ t: now(), device, type: 'type', box: (await target.boundingBox())! });
		await target.pressSequentially(text, { delay: 28 });
	}

	async focus(device: string, name: string, target: Locator) {
		this.events.push({ t: now(), device, type: 'focus', name, box: (await target.boundingBox())! });
	}

	async finish() {
		for (const stop of this.stops) await stop();
		writeFileSync(join(footage, this.scene, 'timeline.json'), JSON.stringify(this.events, null, '\t'));
	}
}
