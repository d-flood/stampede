import type { Item, ToolCall } from '#lib/device.svelte.ts';
import type { Component } from 'svelte';
import FilePlus from '~icons/ph/file-plus';
import FileText from '~icons/ph/file-text';
import Globe from '~icons/ph/globe';
import Link from '~icons/ph/link';
import ListChecks from '~icons/ph/list-checks';
import MagnifyingGlass from '~icons/ph/magnifying-glass';
import PencilSimple from '~icons/ph/pencil-simple';
import Plug from '~icons/ph/plug';
import Robot from '~icons/ph/robot';
import Terminal from '~icons/ph/terminal';
import Wrench from '~icons/ph/wrench';

export const icons: Record<ToolCall['kind'] | 'command' | 'edit' | 'write' | 'todos' | 'subagent', Component> = {
	read: FileText,
	search: MagnifyingGlass,
	fetch: Link,
	web: Globe,
	command: Terminal,
	edit: PencilSimple,
	write: FilePlus,
	mcp: Plug,
	other: Wrench,
	todos: ListChecks,
	subagent: Robot
};

export function icon(item: Item): Component | undefined {
	if (item.kind === 'tools') {
		const kinds = new Set(item.calls.map((call) => call.kind));
		return icons[kinds.size === 1 ? [...kinds][0] : 'other'];
	}
	if (item.kind === 'command' || item.kind === 'subagent') return icons[item.kind];
	if (item.kind === 'edit' || item.kind === 'canvas') return icons[item.write ? 'write' : 'edit'];
}
