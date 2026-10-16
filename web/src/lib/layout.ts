import { claimDeviceId, deviceId, type Chat, type Terminal } from '#lib/device.svelte.ts';
import { OpenConfig, OpenDiff, OpenFile, type Layout as DiffMode } from '#lib/files.svelte.ts';
import type { Commit } from '#lib/git.ts';
import type { Section } from '#lib/shell/Sidebar.svelte';
import type { Split, TabGroup } from '#lib/split.ts';

export type TabRef =
	| { kind: 'terminal'; id: string }
	| { kind: 'chat'; id: string }
	| { kind: 'file'; path: string }
	| { kind: 'config' }
	| { kind: 'diff'; path: string; against: 'HEAD' | Commit; from?: string };

type Half = { tabs: TabRef[]; active: TabRef | null };

export type SplitState = { left: Half; right: Half; ratio: number };

export type Layout = {
	focusedProject: string | null;
	projects: Record<string, { tabs: TabRef[]; active: TabRef | null; split: SplitState | null }>;
	sidebar: { open: boolean; section: Section };
	diffModes: Record<string, DiffMode>;
};

export type Workspace = {
	chosen?: string;
	terminals: Record<string, Terminal[]>;
	chats: Record<string, Chat[]>;
	files: Record<string, OpenFile[]>;
	chosenTabs: Record<string, string>;
	splits: Record<string, Split>;
	sidebar: Layout['sidebar'];
};

let saved: Layout | null = null;
let timer: ReturnType<typeof setTimeout> | undefined;

function url(path: string) {
	return `/api/devices/${deviceId()}/${path}`;
}

export async function loadLayout() {
	await claimDeviceId();
	const response = await fetch(url('layout'));
	saved = response.ok ? await response.json() : null;
}

export function saveLayout(layout: Layout) {
	clearTimeout(timer);
	timer = setTimeout(() => putLayout(layout), 500);
}

export async function putLayout(layout: Layout) {
	clearTimeout(timer);
	await fetch(url('layout'), { method: 'PUT', body: JSON.stringify(layout) }).catch(() => {});
}

export function sendClosing(layout: Layout) {
	clearTimeout(timer);
	navigator.sendBeacon(url('closing'), JSON.stringify(layout));
}

function ref(file: OpenFile): TabRef | null {
	if (file instanceof OpenConfig) return { kind: 'config' };
	if (!(file instanceof OpenDiff)) return { kind: 'file', path: file.path };
	if (file.against === 'disk') return null;
	return { kind: 'diff', path: file.path, against: file.against, from: file.from };
}

function reopen(projectId: string, ref: TabRef): OpenFile | undefined {
	switch (ref.kind) {
		case 'file':
			return new OpenFile(projectId, ref.path);
		case 'config':
			return new OpenConfig(projectId);
		case 'diff':
			return new OpenDiff(projectId, ref.path, ref.against, ref.from);
	}
}

export function capture(workspace: Workspace): Layout {
	const projectIds = new Set([
		...Object.keys(workspace.files),
		...Object.keys(workspace.chosenTabs),
		...Object.keys(workspace.splits)
	]);
	const projects: Layout['projects'] = {};
	const diffModes: Layout['diffModes'] = {};
	for (const projectId of projectIds) {
		const files = workspace.files[projectId] ?? [];
		const terminals = workspace.terminals[projectId] ?? [];
		const chats = workspace.chats[projectId] ?? [];
		const find = (id: string): TabRef | null => {
			const file = files.find((file) => file.id === id);
			if (file) return ref(file);
			return chats.some((chat) => chat.id === id) ? { kind: 'chat', id } : { kind: 'terminal', id };
		};
		const half = ({ tabs, active }: TabGroup): Half => ({
			tabs: tabs.flatMap((id) => find(id) ?? []),
			active: active ? find(active) : null
		});
		const active = workspace.chosenTabs[projectId];
		const split = workspace.splits[projectId];
		projects[projectId] = {
			tabs: [
				...terminals.map(({ id }): TabRef => ({ kind: 'terminal', id })),
				...chats.map(({ id }): TabRef => ({ kind: 'chat', id })),
				...files.flatMap((file) => ref(file) ?? [])
			],
			active: active ? find(active) : null,
			split: split ? { left: half(split.left), right: half(split.right), ratio: split.ratio } : null
		};
		for (const file of files) if (file instanceof OpenDiff) diffModes[file.id] = file.layout;
	}
	return { focusedProject: workspace.chosen ?? null, projects, sidebar: workspace.sidebar, diffModes };
}

export function restore() {
	const files: Workspace['files'] = {};
	const chosenTabs: Workspace['chosenTabs'] = {};
	const splits: Workspace['splits'] = {};
	for (const [projectId, project] of Object.entries(saved?.projects ?? {})) {
		const id = (ref: TabRef) => (ref.kind === 'terminal' || ref.kind === 'chat' ? ref.id : reopen(projectId, ref)!.id);
		const half = ({ tabs, active }: Half): TabGroup => ({ tabs: tabs.map(id), active: active ? id(active) : undefined });
		files[projectId] = project.tabs.flatMap((ref) => reopen(projectId, ref) ?? []);
		for (const file of files[projectId]) {
			if (file instanceof OpenDiff) file.layout = saved?.diffModes[file.id] ?? file.layout;
		}
		if (project.active) chosenTabs[projectId] = id(project.active);
		if (project.split) {
			const { left, right, ratio } = project.split;
			splits[projectId] = { left: half(left), right: half(right), ratio };
		}
	}
	return { chosen: saved?.focusedProject ?? undefined, files, chosenTabs, splits, sidebar: saved?.sidebar };
}
