import { ago, logos } from '#lib/chat/sessions.ts';
import { runsOn, type Rig } from '#lib/config.svelte.ts';
import type { Session, Setting, Settings } from '#lib/device.svelte.ts';
import type { Component } from 'svelte';
import FileCode from '~icons/ph/file-code';
import Globe from '~icons/ph/globe';
import TerminalWindow from '~icons/ph/terminal-window';

const RECENT = 5;

export type TabItem = { label: string; icon?: Component; hint?: string; shortcut?: string; onselect: () => void };
export type RigItem = TabItem & { rig: Rig; model: string; onprompt(text: string, settings?: Settings): void };
export type TabItems = { rigs: RigItem[]; tools: TabItem[]; sessions: TabItem[]; search?: TabItem };

type Actions = {
	launch(rig: Rig, text?: string, settings?: Partial<Record<Setting, string>>): void;
	terminal(): void;
	browser(): void;
	files(): void;
	reopen(session: Session): void;
	search(): void;
};

export function tabItems(rigs: Rig[], sessions: Session[], actions: Actions, host?: string): TabItems {
	const recent = sessions.slice(0, RECENT);
	return {
		rigs: rigs.map((rig, index): RigItem => ({
			rig,
			label: rig.name,
			icon: logos[rig.agent],
			shortcut: index < 9 ? `Ctrl+Shift+${index + 1}` : undefined,
			model: [rig.model, rig.effort].filter(Boolean).join(', '),
			onselect: () => actions.launch(rig),
			onprompt: (text, settings) => actions.launch(rig, text, settings)
		})).filter(({ rig }) => runsOn(host, rig.agent)),
		tools: [
			{ label: 'Terminal', shortcut: 'Ctrl+Shift+T', icon: TerminalWindow, onselect: actions.terminal },
			{ label: 'Browser', icon: Globe, onselect: actions.browser },
			{ label: 'Open file', shortcut: 'Ctrl+Shift+O', icon: FileCode, onselect: actions.files }
		],
		sessions: recent.map((session) => ({ label: session.title, icon: logos[session.agent], hint: ago(session.updatedAt), onselect: () => actions.reopen(session) })),
		search: recent.length ? { label: 'Search sessions…', onselect: actions.search } : undefined
	};
}

export function menuItems({ rigs, tools, sessions, search }: TabItems) {
	return [
		...(rigs.length ? [{ heading: 'Rigs', items: rigs }] : []),
		{ heading: 'Open', items: tools },
		...(search ? [{ heading: 'Sessions', items: [...sessions, search] }] : [])
	];
}
