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

export function color(main: Item[], path: string[]) {
	const index = Math.max(0, subagents(main).findIndex((placed) => same(placed.path, path)));
	return `var(--subagent-${(index % 6) + 1})`;
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

export function duration(milliseconds: number) {
	const seconds = Math.round(milliseconds / 1000);
	const minutes = Math.floor(seconds / 60);
	return minutes >= 60 ? `${Math.floor(minutes / 60)}h ${minutes % 60}m` : minutes ? `${minutes}m ${seconds % 60}s` : `${seconds}s`;
}

export function tools(count: number) {
	return `${count} ${count === 1 ? 'tool' : 'tools'}`;
}

export function totals({ durationMs, tools: used, tokens }: NonNullable<Subagent['totals']>) {
	return [duration(durationMs), tools(used), tokens !== undefined && `${count(tokens)} tokens`].filter(Boolean).join(' · ');
}
