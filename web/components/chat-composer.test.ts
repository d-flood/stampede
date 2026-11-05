import ChatComposer from '#lib/chat/ChatComposer.svelte';
import type { Capabilities, Chat, Device } from '#lib/device.svelte.ts';
import { expect, test, vi } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

const phone = matchMedia('(pointer: coarse) and (max-width: 767px)').matches;
const png = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==';
const pixels = () => Uint8Array.from(atob(png), (char) => char.charCodeAt(0));
const capabilities: Capabilities = { rewindsFiles: false, models: [], efforts: [], speeds: [], ultracode: false, modes: [], commands: [] };

async function mount(overrides: Partial<Chat> = {}) {
	const send = vi.fn();
	const chat: Chat = { id: crypto.randomUUID(), projectId: 'project', rig: 'Fake Claude', agent: 'claude-code', status: 'done', context: null, tokens: null, queued: [], queueRevision: 0, todos: [], answeredQuestions: [], settings: {}, ...overrides };
	const view = await render(ChatComposer, {
		device: { send } as unknown as Device,
		chat,
		live: true,
		capabilities,
		choices: { model: [], effort: [], speed: [], ultracode: [], mode: [] },
		branches: [],
		history: [],
		shelling: false,
		onrun: vi.fn()
	});
	return {
		view,
		send,
		chat,
		composer: view.getByRole('textbox', { name: 'Composer' }),
		attachments: view.getByLabelText('Attachments', { exact: true }),
		sendButton: view.getByRole('button', { name: 'Send', exact: true })
	};
}

function paste(composer: Element, files: { name: string; type?: string; size?: number }[], text = '') {
	const clipboardData = new DataTransfer();
	if (text) clipboardData.setData('text/plain', text);
	for (const file of files) clipboardData.items.add(new File([file.size === undefined ? pixels() : new Uint8Array(file.size)], file.name, { type: file.type ?? 'image/png' }));
	return composer.dispatchEvent(new ClipboardEvent('paste', { clipboardData, bubbles: true, cancelable: true }));
}

test.skipIf(phone)('text paste stays native and pasted images can be removed', async () => {
	const { view, composer, attachments } = await mount();
	expect(paste(composer.element(), [], 'plain text')).toBe(true);
	expect(paste(composer.element(), [{ name: 'notes.txt', type: 'text/plain' }])).toBe(true);
	await expect.element(attachments).not.toBeInTheDocument();
	await composer.click();
	await navigator.clipboard.writeText('plain text\nsecond line');
	await userEvent.keyboard('{Control>}v{/Control}');
	await expect.element(composer).toHaveValue('plain text\nsecond line');
	paste(composer.element(), [{ name: 'remove.png' }]);
	await expect.element(view.getByRole('img', { name: 'remove.png' })).toBeVisible();
	await view.getByRole('button', { name: 'Remove remove.png', exact: true }).click();
	await expect.element(attachments).not.toBeInTheDocument();
});

test('pasted images share picker limits and reject overlapping reads', async () => {
	const { view, composer, attachments } = await mount();
	const alert = view.getByRole('alert');
	paste(composer.element(), [{ name: 'vector.svg', type: 'image/svg+xml' }]);
	await expect.element(alert).toHaveTextContent('Choose PNG, JPEG, GIF or WebP images.');
	paste(composer.element(), [{ name: 'large.png', size: 5 * 1024 * 1024 + 1 }]);
	await expect.element(alert).toHaveTextContent('Each image must be 5 MB or smaller.');
	paste(composer.element(), Array.from({ length: 5 }, (_, index) => ({ name: `${index}.png`, size: 4 * 1024 * 1024 + 1 })));
	await expect.element(alert).toHaveTextContent('Keep attachments under 20 MB per message.');
	await expect.element(attachments).not.toBeInTheDocument();
	await userEvent.upload(view.getByLabelText('Attach images', { exact: true }), new File([pixels()], 'picked.png', { type: 'image/png' }));
	await expect.element(view.getByRole('img', { name: 'picked.png' })).toBeVisible();
	const ten = Array.from({ length: 10 }, (_, index) => ({ name: `${index}.png` }));
	paste(composer.element(), ten);
	paste(composer.element(), ten);
	await expect.poll(() => attachments.element().querySelectorAll('img').length).toBe(11);
	await expect.element(view.getByRole('button', { name: 'Attach image', exact: true })).toBeEnabled();
	paste(composer.element(), Array.from({ length: 10 }, (_, index) => ({ name: `extra-${index}.png` })));
	await expect.element(alert).toHaveTextContent('Attach up to 20 images.');
	expect(attachments.element().querySelectorAll('img')).toHaveLength(11);
});

test.runIf(phone)('mobile attachment controls leave the previews unobscured', async () => {
	const { view, composer, attachments } = await mount();
	paste(composer.element(), [{ name: 'first.png' }, { name: 'second.png' }]);
	await expect.poll(() => attachments.element().querySelectorAll('img').length).toBe(2);
	const { innerWidth: width, innerHeight: height } = window;
	for (const narrow of [448, 320]) {
		await page.viewport(narrow, height);
		for (const name of ['first.png', 'second.png']) {
			const image = view.getByRole('img', { name, exact: true }).element().getBoundingClientRect();
			const remove = view.getByRole('button', { name: `Remove ${name}`, exact: true }).element().getBoundingClientRect();
			expect(Math.max(0, Math.min(image.right, remove.right) - Math.max(image.left, remove.left)) * Math.max(0, Math.min(image.bottom, remove.bottom) - Math.max(image.top, remove.top))).toBe(0);
			expect(remove.width).toBeGreaterThanOrEqual(48);
			expect(remove.height).toBeGreaterThanOrEqual(48);
		}
		expect(view.container.scrollWidth).toBeLessThanOrEqual(view.container.clientWidth);
	}
	await page.viewport(width, height);
	await view.getByRole('button', { name: 'Remove first.png', exact: true }).click();
	await expect.element(view.getByRole('img', { name: 'first.png', exact: true })).not.toBeInTheDocument();
	await expect.element(view.getByRole('img', { name: 'second.png', exact: true })).toBeVisible();
});

test('a cold prompt cache confirms before re-reading a large context', async () => {
	const expires = Date.now() - 3 * 24 * 60 * 60 * 1000;
	const { view, chat, send, composer, sendButton } = await mount({ context: { used: 512_000, window: 1_000_000, cache: { expires, then: 'cold' } } });
	if (!phone) await expect.element(view.getByRole('meter', { name: 'Context' })).toHaveAttribute('aria-valuetext', '512k / 1M (51%), cache cold');
	await composer.fill('Anything else?');
	await sendButton.click();
	const dialog = page.getByRole('dialog', { name: 'Cache cold' });
	await expect.element(dialog.getByText('The prompt cache expired 3 days ago, so this message re-reads all 512k tokens of context uncached.')).toBeVisible();
	expect(send).not.toHaveBeenCalled();
	await dialog.getByRole('button', { name: 'Cancel' }).click();
	await expect.element(dialog).not.toBeInTheDocument();
	await expect.element(composer).toHaveValue('Anything else?');
	await expect.element(composer).toHaveFocus();
	await sendButton.click();
	await dialog.getByRole('button', { name: 'Send anyway' }).click();
	await expect.element(dialog).not.toBeInTheDocument();
	await expect.element(composer).toHaveValue('');
	expect(send).toHaveBeenCalledExactlyOnceWith({ type: 'chat.send', chatId: chat.id, text: 'Anything else?', queue: false, images: [] });
});
