import type { Component } from 'svelte';

export type Status = 'working' | 'waiting' | 'done';

export type TabData = {
	variant: 'agent' | 'terminal' | 'editor' | 'diff';
	label: string;
	agent?: 'claude-code' | 'codex';
	status?: Status;
	added?: number;
	removed?: number;
};

export type TabProps = TabData & { active?: boolean };

export type TabComponent = Component<TabProps>;

export const projects = [
	{ name: 'oikonomos', monogram: 'Oi', path: '~/repos/oikonomos', branch: 'main', agents: 4, attention: 2 },
	{ name: 'botley', monogram: 'Bo', path: '~/repos/botley', branch: 'ticket-loop', agents: 1, attention: 0 },
	{ name: 'iiif-atlas', monogram: 'Ia', path: '~/repos/iiif-atlas', branch: 'main', agents: 0, attention: 0 }
];

export const tabs: TabData[] = [
	{ variant: 'agent', label: 'claude-yolo', status: 'working' },
	{ variant: 'agent', label: 'claude-review', status: 'waiting' },
	{ variant: 'agent', label: 'claude-docs', status: 'done' },
	{ variant: 'agent', agent: 'codex', label: 'codex', status: 'working' },
	{ variant: 'terminal', label: 'zsh' },
	{ variant: 'editor', label: 'terminals.rs' },
	{ variant: 'diff', label: 'lib.rs', added: 6, removed: 2 }
];

export const split: TabData[] = [
	{ variant: 'diff', label: 'device.ts', added: 14, removed: 3 },
	{ variant: 'editor', label: 'config.svelte.ts' }
];

export const attention = [
	{ tab: tabs[1], project: 'oikonomos', title: 'Review reconnect backoff', note: 'Wants to edit web/src/lib/device.ts', time: 'now' },
	{ tab: tabs[2], project: 'oikonomos', title: 'Draft ADR 0008', note: 'Finished its turn', time: '4 min' }
];

export const changes = [
	{ dir: 'web/src/lib/', name: 'device.ts', status: 'M', added: 14, removed: 3 },
	{ dir: 'crates/oikonomos/src/', name: 'lib.rs', status: 'M', added: 6, removed: 2 },
	{ dir: 'crates/oikonomos/src/', name: 'terminals.rs', status: 'M', added: 41, removed: 17 },
	{ dir: 'crates/oikonomos/tests/', name: 'reconnect_backoff.rs', status: 'A', added: 88, removed: 0 },
	{ dir: 'web/src/lib/', name: 'socket.ts', status: 'D', added: 0, removed: 52 }
];

export const keys = ['Esc', 'Ctrl', 'Tab', '⇧Tab', '←', '↓', '↑', '→', '|', '/'];
