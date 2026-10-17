import type { Chat, Item, Status } from '#lib/device.svelte.ts';

export type AgentSession = { id: string; label: string; task: string; status: Status; activity?: string | null; items: Item[] };

export function agentSessions(chat: Chat, items: Item[]): AgentSession[] {
	const sessions: AgentSession[] = [{ id: '', label: 'Main', task: chat.rig, status: chat.status, activity: chat.activity, items }];
	function visit(items: Item[], parent: string, depth: number) {
		items.forEach((item, index) => {
			if (item.kind !== 'subagent') return;
			const id = `${parent}/${index}`;
			sessions.push({ id, label: `${'↳ '.repeat(depth)}${item.name || item.task}`, task: item.task, status: item.status, activity: item.activity, items: item.items });
			visit(item.items, id, depth + 1);
		});
	}
	visit(items, '', 1);
	return sessions;
}
