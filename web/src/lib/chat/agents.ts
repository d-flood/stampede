import { count } from '#lib/chat/count.ts';
import type { Item, Subagent } from '#lib/device.svelte.ts';
import { createSubscriber } from 'svelte/reactivity';

export type Placed = { path: string[]; subagent: Subagent };

export function subagents(items: Item[], parent: string[] = []): Placed[] {
	return items.flatMap((item) => {
		if (item.kind !== 'subagent') return [];
		const path = [...parent, item.id];
		return [{ path, subagent: item }, ...subagents(item.items, path)];
	});
}

export function same(a: string[], b: string[]) {
	return a.length === b.length && a.every((id, index) => id === b[index]);
}

const tick = createSubscriber((update) => {
	const timer = setInterval(update, 1000);
	return () => clearInterval(timer);
});

export function clock(startedAt: number) {
	tick();
	const seconds = Math.max(0, Math.floor((Date.now() - startedAt) / 1000));
	const pad = (value: number) => String(value).padStart(2, '0');
	const minutes = Math.floor(seconds / 60);
	if (minutes < 60) return `${minutes}:${pad(seconds % 60)}`;
	return `${Math.floor(minutes / 60)}:${pad(minutes % 60)}:${pad(seconds % 60)}`;
}

export function totals({ durationMs, tools, tokens }: NonNullable<Subagent['totals']>) {
	const seconds = Math.round(durationMs / 1000);
	const minutes = Math.floor(seconds / 60);
	const duration = minutes >= 60 ? `${Math.floor(minutes / 60)}h ${minutes % 60}m` : minutes ? `${minutes}m ${seconds % 60}s` : `${seconds}s`;
	return [duration, `${tools} ${tools === 1 ? 'tool' : 'tools'}`, tokens !== undefined && `${count(tokens)} tokens`].filter(Boolean).join(' · ');
}
