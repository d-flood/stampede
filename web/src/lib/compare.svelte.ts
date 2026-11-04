import { RequestError, type Selection } from '#lib/files.svelte.ts';
import { compare, type Comparison } from '#lib/git.ts';

/** One Comparison's latest result, shared by the Compare section and its Diff tab. */
export class Compared {
	readonly projectId: string;
	readonly selection: Selection;
	result = $state.raw<Comparison>();
	error = $state.raw<{ status: number; message: string }>();
	#loads = 0;

	constructor(projectId: string, selection: Selection) {
		this.projectId = projectId;
		this.selection = selection;
	}

	/** Asks again, replacing the result only when a branch or their merge base moved. */
	async load() {
		const generation = ++this.#loads;
		try {
			const next = await compare(this.projectId, this.selection.target, this.selection.source);
			if (generation !== this.#loads) return;
			this.error = undefined;
			const now = this.result;
			if (now?.target !== next.target || now.source !== next.source || now.mergeBase !== next.mergeBase) this.result = next;
		} catch (error) {
			if (generation !== this.#loads) return;
			this.result = undefined;
			this.error = { status: error instanceof RequestError ? error.status : 0, message: error instanceof Error ? error.message : String(error) };
		}
	}
}

const compared = new Map<string, Compared>();

export function comparison(projectId: string, selection: Selection): Compared {
	const key = JSON.stringify([projectId, selection.target, selection.source]);
	let found = compared.get(key);
	if (!found) compared.set(key, (found = new Compared(projectId, { target: selection.target, source: selection.source })));
	return found;
}
