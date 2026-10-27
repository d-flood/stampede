import type { Item } from '#lib/device.svelte.ts';

export type Shell = { id: string; owner: string; item: Extract<Item, { kind: 'command' }> };

export function commands(items: Item[], parent = '', owner = 'Main'): Shell[] {
	return items.flatMap((item, index) => {
		const id = `${parent}/${index}`;
		if (item.kind === 'subagent') return commands(item.items, id, item.name || item.task);
		return item.kind === 'command' ? [{ id, owner, item }] : [];
	});
}
