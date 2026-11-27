import type { Chat, Session } from '#lib/device.svelte.ts';
import ChatList from '#lib/shell/ChatList.svelte';
import { expect, test, vi } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

const chat = (id: string, status: Chat['status'], title: string): Chat => ({
	id, projectId: 'p', rig: 'Claude', agent: 'claude-code', status, created: Date.now() * 1e6, title,
	context: null, tokens: null, queued: [], queueRevision: 0, todos: [], answeredQuestions: [], settings: {}
});
const sessions: Session[] = [{ key: { agent: 'claude-code', store: '/claude', sessionId: 's' }, project: { id: 'p', path: '/project' }, title: 'Past work', updatedAt: Date.now() }];

test('the Chats list leads with waiting Chats, then running, then the rest, and its button carries a dot only while one waits', async () => {
	const chats = [chat('a', 'done', 'Viewed'), chat('b', 'working', 'Busy'), chat('c', 'done', 'Unseen'), chat('d', 'waiting', 'Blocked')];
	const props = { chats, status: (chat: Chat) => chat.status, sessions, reading: false, onopen: vi.fn(), onchat: vi.fn(), onsession: vi.fn() };
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
