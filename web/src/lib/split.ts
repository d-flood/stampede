export type Side = 'left' | 'right';

export type TabGroup = { tabs: string[]; active?: string };

export type Split = { left: TabGroup; right: TabGroup; ratio: number };

export function arrange(split: Split | undefined, tabIds: string[], active: string | undefined): Split | undefined {
	if (!split) return;
	const right = split.right.tabs.filter((id) => tabIds.includes(id));
	const listed = split.left.tabs.filter((id) => tabIds.includes(id) && !right.includes(id));
	const left = [...listed, ...tabIds.filter((id) => !listed.includes(id) && !right.includes(id))];
	if (!left.length || !right.length) return;
	return { left: group(left, active, split.left.active), right: group(right, active, split.right.active), ratio: split.ratio };
}

function group(tabs: string[], ...preferred: (string | undefined)[]): TabGroup {
	return { tabs, active: preferred.find((id) => id && tabs.includes(id)) ?? tabs.at(-1) };
}

export function neighbor(tabs: string[], tabId: string): string | undefined {
	const index = tabs.indexOf(tabId);
	return tabs[index + 1] ?? tabs[index - 1];
}
