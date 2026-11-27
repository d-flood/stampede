import type { Chat, ClientMessage, Device, FoundSession, ServerMessage, Session } from '#lib/device.svelte.ts';
import ChatList from '#lib/shell/ChatList.svelte';
import { expect, test, vi } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

const chat = (id: string, status: Chat['status'], title: string): Chat => ({
	id, projectId: 'p', rig: 'Claude', agent: 'claude-code', status, created: Date.now() * 1e6, title,
	context: null, tokens: null, queued: [], queueRevision: 0, todos: [], answeredQuestions: [], settings: {}
});
const sessions: Session[] = [{ key: { agent: 'claude-code', store: '/claude', sessionId: 's' }, project: { id: 'p', path: '/project' }, title: 'Past work', updatedAt: Date.now() }];
const session = (sessionId: string, title: string, project = { id: 'p', path: '/project' }): Session => ({ key: { agent: 'claude-code', store: '/claude', sessionId }, project, title, updatedAt: Date.now() });

function host() {
	const sent: ClientMessage[] = [];
	let heard: ((message: ServerMessage) => void) | undefined;
	const device = { send: (message: ClientMessage) => sent.push(message), listen: (listener: { message(message: ServerMessage): void }) => ((heard = listener.message), () => {}) } as unknown as Device;
	return { search: { device, projectId: 'p', focus: 'p' }, sent, reply: (message: ServerMessage) => heard?.(message) };
}

test('the Chats list leads with waiting Chats, then running, then the rest, and its button carries a dot only while one waits', async () => {
	const chats = [chat('a', 'done', 'Viewed'), chat('b', 'working', 'Busy'), chat('c', 'done', 'Unseen'), chat('d', 'waiting', 'Blocked')];
	const props = { chats, status: (chat: Chat) => chat.status, sessions, reading: false, search: host().search, onopen: vi.fn(), onchat: vi.fn(), onsession: vi.fn() };
	const view = await render(ChatList, { ...props, waiting: new Set(['c', 'd']) });
	const button = page.getByRole('button', { name: 'Chats', exact: true });
	expect(button.element().querySelector('.dot')).not.toBeNull();
	await button.click();
	const list = page.getByRole('dialog', { name: 'Chats' });
	const groups = list.getByRole('group');
	await expect.element(groups.nth(3)).toBeVisible();
	expect(groups.elements().map((group) => document.getElementById(group.getAttribute('aria-labelledby') ?? '')?.textContent)).toEqual(['Waiting', 'Running', 'Open', 'Recent sessions']);
	expect(groups.nth(0).getByRole('option').elements().map((option) => option.textContent?.match(/Unseen|Blocked/)?.[0])).toEqual(['Unseen', 'Blocked']);
	await list.getByRole('combobox').fill('B');
	await expect.element(list.getByRole('group', { name: 'Open Chats' })).toBeVisible();
	await expect.element(list.getByRole('group', { name: 'Waiting' })).not.toBeInTheDocument();
	await userEvent.keyboard('{Escape}');

	await view.rerender({ ...props, waiting: new Set() });
	expect(button.element().querySelector('.dot')).toBeNull();
	await button.click();
	await expect.element(list.getByRole('group', { name: 'Running' })).toBeVisible();
	await view.rerender({ ...props, chats: [chats[0]], waiting: new Set() });
	await expect.element(list.getByRole('group', { name: 'Open Chats' })).toBeVisible();
});

test('a query shows the matches the Host found with highlighted excerpts, then title matches it missed, then other Projects', async () => {
	const { search, sent, reply } = host();
	const held = [...sessions, session('t', 'Tokenizer notes')];
	await render(ChatList, { chats: [], status: (chat: Chat) => chat.status, waiting: new Set<string>(), sessions: held, reading: false, search, onopen: vi.fn(), onchat: vi.fn(), onsession: vi.fn() });
	await page.getByRole('button', { name: 'Chats', exact: true }).click();
	const list = page.getByRole('dialog', { name: 'Chats' });
	await list.getByRole('combobox').fill('token');
	await expect.poll(() => sent.at(-1)).toMatchObject({ type: 'sessions.search', projectId: 'p', query: 'token' });
	const { searchId } = sent.at(-1) as Extract<ClientMessage, { type: 'sessions.search' }>;
	const results: FoundSession[] = [
		{ ...session('d', 'Search design'), matches: 3, excerpt: { who: 'agent', text: '… a trigram tokenizer grows', ranges: [[12, 21]] } },
		{ ...session('e', 'Sidebar perf', { id: 'q', path: '/work/botley' }), ended: 'aged', matches: 1, excerpt: { who: 'user', text: 'which tokenizer?', ranges: [[6, 15]] } }
	];
	reply({ type: 'sessions.found', searchId, results, coverage: { indexed: 4, total: 4, stale: [], ended: 1, bytes: { index: 0, archive: 0 } } });

	const here = list.getByRole('group', { name: 'Sessions' }).getByRole('option');
	await expect.element(here.nth(1)).toBeVisible();
	expect(here.first().element().textContent).toMatch(/Search design.*Claude Code … a trigram tokenizer grows\s*\+2/);
	expect(here.first().element().querySelector('mark')?.textContent).toBe('tokenizer');
	expect(here.elements().map((option) => option.textContent?.match(/Search design|Tokenizer notes/)?.[0])).toEqual(['Search design', 'Tokenizer notes']);
	const other = list.getByRole('group', { name: 'Other Projects' }).getByRole('option');
	expect(other.element().textContent).toMatch(/Sidebar perf.*botley.*Ended.*You which tokenizer\?/);
	await expect.element(other).toHaveAttribute('aria-disabled', 'true');
});
