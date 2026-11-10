import { ago, logos } from '#lib/chat/sessions.ts';
import type { Rig } from '#lib/config.svelte.ts';
import type { Session } from '#lib/device.svelte.ts';
import type { Component } from 'svelte';
import FileCode from '~icons/ph/file-code';
import Globe from '~icons/ph/globe';
import TerminalWindow from '~icons/ph/terminal-window';

const RECENT = 5;

export type TabItem = { label: string; icon?: Component; hint?: string; onselect: () => void };
export type RigItem = TabItem & { model: string };
export type TabItems = { rigs: RigItem[]; tools: TabItem[]; sessions: TabItem[]; search?: TabItem };

type Actions = {
	launch(rig: Rig): void;
	terminal(): void;
	browser(): void;
	files(): void;
	reopen(session: Session): void;
	search(): void;
};

export function tabItems(rigs: Rig[], sessions: Session[], actions: Actions): TabItems {
	const recent = sessions.slice(0, RECENT);
	return {
		rigs: rigs.map((rig, index) => ({
			label: rig.name,
			icon: logos[rig.agent],
			hint: index < 9 ? `Ctrl+Shift+${index + 1}` : `${index + 1}`,
			model: [rig.model ?? 'Default model', rig.effort].filter(Boolean).join(', '),
			onselect: () => actions.launch(rig)
		})),
		tools: [
			{ label: 'Terminal', hint: 'Ctrl+Shift+T', icon: TerminalWindow, onselect: actions.terminal },
			{ label: 'Browser', icon: Globe, onselect: actions.browser },
			{ label: 'Open file', hint: 'Ctrl+Shift+O', icon: FileCode, onselect: actions.files }
		],
		sessions: recent.map((session) => ({ label: session.title, icon: logos[session.agent], hint: ago(session.updatedAt), onselect: () => actions.reopen(session) })),
		search: recent.length ? { label: 'Search sessions…', onselect: actions.search } : undefined
	};
}

export function menuItems({ rigs, tools, sessions, search }: TabItems) {
	return [
		...(rigs.length ? [{ heading: 'Rigs', items: rigs }] : []),
		...tools,
		...(search ? [{ heading: 'Sessions', items: [...sessions, search] }] : [])
	];
}
