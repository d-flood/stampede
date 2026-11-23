import type { Item, ToolCall } from '#lib/device.svelte.ts';

export type Chip = { path: string; added: number; removed: number; canvas?: true };

export function summarize(items: Item[]): string[] {
	const calls = items.flatMap((item) => (item.kind === 'tools' ? item.calls : []));
	const commands = items.filter((item) => item.kind === 'command');
	const failed = commands.filter((item) => item.status === 'failed').length;
	const count = (...kinds: ToolCall['kind'][]) => calls.filter((call) => kinds.includes(call.kind)).length;
	const counts: [number, string, string][] = [
		[chips(items).length, 'edited', 'file'],
		[commands.length, 'ran', 'command'],
		[count('read'), 'read', 'file'],
		[count('search'), 'searched', 'time'],
		[count('fetch'), 'fetched', 'page'],
		[count('web'), 'searched the web', 'time'],
		[count('mcp', 'other'), 'used', 'tool']
	];
	const approvals = calls.filter((call) => call.kind === 'approval');
	const denied = approvals.filter((call) => call.name === 'denied').length;
	const timedOut = approvals.filter((call) => call.name === 'timedOut').length;
	const segments = counts.filter(([count]) => count).map(([count, verb, noun]) => `${verb} ${count} ${noun}${count === 1 ? '' : 's'}`);
	if (failed) segments.push(`${failed} failed`);
	if (denied) segments.push(`${denied} ${denied === 1 ? 'approval' : 'approvals'} denied`);
	if (timedOut) segments.push(`${timedOut} ${timedOut === 1 ? 'approval' : 'approvals'} timed out`);
	if (!segments.length) {
		const thinking = items.filter((item) => item.kind === 'thinking');
		if (!thinking.length && approvals.length) {
			return [`Reviewed ${approvals.length} ${approvals.length === 1 ? 'approval' : 'approvals'}`];
		}
		const seconds = thinking.reduce((total, item) => total + item.seconds, 0);
		return [seconds ? `Thought for ${seconds}s` : 'Thought briefly'];
	}
	segments[0] = segments[0].charAt(0).toUpperCase() + segments[0].slice(1);
	return segments;
}

export function chips(items: Item[]): Chip[] {
	const chips = new Map<string, Chip>();
	for (const item of items) {
		if (item.kind !== 'edit' && item.kind !== 'canvas') continue;
		const chip: Chip = item.kind === 'edit' ? { path: item.path, added: 0, removed: 0 } : { path: item.canvas, added: 0, removed: 0, canvas: true };
		const key = `${item.kind}:${chip.path}`;
		const total = chips.get(key) ?? chip;
		total.added += item.added;
		total.removed += item.removed;
		chips.set(key, total);
	}
	return [...chips.values()];
}
