export type Side = 'left' | 'right';

export type TabGroup = { tabs: string[]; active?: string };

export type Split = { left: TabGroup; right: TabGroup; ratio: number };

export function arrange(split: Split | undefined, tabIds: string[], active: string | undefined, closing: Set<string>): Split | undefined {
	if (!split) return;
	const right = split.right.tabs.filter((id) => tabIds.includes(id));
	const listed = split.left.tabs.filter((id) => tabIds.includes(id) && !right.includes(id));
	const left = [...listed, ...tabIds.filter((id) => !listed.includes(id) && !right.includes(id))];
	if (!left.length || !right.length) return;
	return { left: group(left, closing, active, split.left.active), right: group(right, closing, active, split.right.active), ratio: split.ratio };
}

function group(tabs: string[], closing: Set<string>, ...preferred: (string | undefined)[]): TabGroup {
	const open = tabs.filter((id) => !closing.has(id));
	return { tabs, active: preferred.find((id) => id && open.includes(id)) ?? open.at(-1) };
}

export function neighbor(tabs: string[], tabId: string, closing: Set<string>): string | undefined {
	const index = tabs.indexOf(tabId);
	const open = (id: string) => !closing.has(id);
	return tabs.slice(index + 1).find(open) ?? tabs.slice(0, index).findLast(open);
}
