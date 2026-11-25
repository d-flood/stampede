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
			if (worktree && 'commit' in worktree) return [id, basename(path)];
			if (worktree) {
				if (worktree.role === 'integration') return [id, `Integration ${worktree.epic}`];
				if ('reviewer' in worktree.role) return [id, `Reviewer ${worktree.role.reviewer} ${worktree.epic}`];
				return [id, `${worktree.epic}/${worktree.role.ticket}`];
			}
			const name = basename(path);
			const collides = own.some((other) => other.id !== id && basename(other.path) === name);
			return [id, collides ? segments(path).slice(-2).join('/') : name];
		})
	);
}

/** The ANSI color, 1–6, a Project's monogram is tinted with. */
export function hue(path: string): number {
	return ([...path].reduce((sum, letter) => sum + letter.charCodeAt(0), 0) % 6) + 1;
}

export function monogram(path: string, length = 2): string {
	const name = basename(path);
	const words = name.split(/[-_.\s]+/).filter(Boolean);
	const letters = words.length > 1 ? words[0][0] + words[1][0] : (words[0] ?? name).slice(0, 2);
	return (letters[0].toUpperCase() + letters.slice(1).toLowerCase()).slice(0, length);
}

export function worktrees(projects: Project[], epics: Record<string, Epic[]>, chats: Chat[], statusOf: (chat: Chat) => Status | 'background', needs: Map<string, number>): Map<string, Tile[]> {
	const rank = (worktree: Worktree) => {
		if (!('epic' in worktree)) return Number.MAX_SAFE_INTEGER;
		if (worktree.role === 'integration') return 0;
		if ('ticket' in worktree.role) return worktree.role.ticket;
		return 1e9 + worktree.role.reviewer;
	};
	const nested = projects.flatMap(({ id, path, worktree }) => (worktree ? [{ id, path, worktree }] : []));
	const tiles = new Map<string, Tile[]>();
	for (const { id, path, worktree } of nested.sort((a, b) => rank(a.worktree) - rank(b.worktree))) {
		const statuses = chats.filter(({ projectId }) => projectId === id).map(statusOf);
		// Waiting shows through the attention count, so it yields to the activity marks.
		const active = (['working', 'background', 'waiting'] as const).find((status) => statuses.includes(status));
		if (!('epic' in worktree)) {
			const mark = monogram('branch' in worktree ? worktree.branch : path);
			tiles.set(worktree.parent, [...(tiles.get(worktree.parent) ?? []), { id, mark, status: active, needs: needs.get(id) }]);
			continue;
		}
		const { parent, epic: slug, role } = worktree;
		const epic = epics[parent]?.find((other) => other.slug === slug);
		const ticket = role !== 'integration' && 'ticket' in role ? epic?.tickets.find(({ number }) => number === role.ticket) : undefined;
		const reviewer = role !== 'integration' && 'reviewer' in role ? epic?.review?.reviewers.find(({ number }) => number === role.reviewer) : undefined;
		const chat =
			role === 'integration'
				? (epic?.tickets.find(({ resolver }) => resolver)?.resolver ?? epic?.review?.remediator?.chat)
				: reviewer
					? reviewer.chat
					: ticket?.claim?.chat;
		const claimed = chats.find((other) => other.id === chat);
		const status = ticket?.releasing ? 'stopping' : (active ?? claimed?.status);
		const mark = role === 'integration' ? 'In' : 'reviewer' in role ? `R${role.reviewer}` : String(role.ticket);
		tiles.set(parent, [...(tiles.get(parent) ?? []), { id, mark, status, needs: needs.get(id) }]);
	}
	return tiles;
}
