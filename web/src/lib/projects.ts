import type { Chat, Epic, Project, Status, Worktree } from '#lib/device.svelte.ts';

export type Tile = { id: string; mark: string; status?: Status | 'background' | 'stopping'; needs?: number };

function segments(path: string): string[] {
	return path.split('/').filter(Boolean);
}

function basename(path: string): string {
	return segments(path).at(-1) ?? '/';
}

export function labels(projects: Project[]): Map<string, string> {
	const own = projects.filter((project) => !project.worktree);
	return new Map(
		projects.map(({ id, path, worktree }) => {
			if (worktree && 'branch' in worktree) return [id, worktree.branch];
			if (worktree) return [id, worktree.role === 'integration' ? `Integration ${worktree.epic}` : `${worktree.epic}/${worktree.role.ticket}`];
			const name = basename(path);
			const collides = own.some((other) => other.id !== id && basename(other.path) === name);
			return [id, collides ? segments(path).slice(-2).join('/') : name];
		})
	);
}

export function monogram(path: string): string {
	const name = basename(path);
	const words = name.split(/[-_.\s]+/).filter(Boolean);
	const letters = words.length > 1 ? words[0][0] + words[1][0] : (words[0] ?? name).slice(0, 2);
	return letters[0].toUpperCase() + letters.slice(1).toLowerCase();
}

export function worktrees(projects: Project[], epics: Record<string, Epic[]>, chats: Chat[], statusOf: (chat: Chat) => Status | 'background', needs: Map<string, number>): Map<string, Tile[]> {
	const rank = (worktree: Worktree) => ('branch' in worktree ? Number.MAX_SAFE_INTEGER : worktree.role === 'integration' ? 0 : worktree.role.ticket);
	const nested = projects.flatMap(({ id, worktree }) => (worktree ? [{ id, worktree }] : []));
	const tiles = new Map<string, Tile[]>();
	for (const { id, worktree } of nested.sort((a, b) => rank(a.worktree) - rank(b.worktree))) {
		const statuses = chats.filter(({ projectId }) => projectId === id).map(statusOf);
		// Waiting shows through the attention count, so it yields to the activity marks.
		const active = (['working', 'background', 'waiting'] as const).find((status) => statuses.includes(status));
		if ('branch' in worktree) {
			tiles.set(worktree.parent, [...(tiles.get(worktree.parent) ?? []), { id, mark: monogram(worktree.branch), status: active, needs: needs.get(id) }]);
			continue;
		}
		const { parent, epic: slug, role } = worktree;
		const tickets = epics[parent]?.find((epic) => epic.slug === slug)?.tickets ?? [];
		const ticket = role === 'integration' ? undefined : tickets.find(({ number }) => number === role.ticket);
		const chat = role === 'integration' ? tickets.find(({ resolver }) => resolver)?.resolver : ticket?.claim?.chat;
		const claimed = chats.find((other) => other.id === chat);
		const status = ticket?.releasing ? 'stopping' : (active ?? claimed?.status);
		const mark = role === 'integration' ? 'In' : String(role.ticket);
		tiles.set(parent, [...(tiles.get(parent) ?? []), { id, mark, status, needs: needs.get(id) }]);
	}
	return tiles;
}
