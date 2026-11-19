export type Action = 'fetch' | 'pull' | 'push' | 'publish' | 'trunk' | 'commit' | 'draft' | 'stage' | 'unstage' | 'discard' | 'stash' | 'unstash' | 'undo';

export type Progress = { action: Action; paths: string[]; output: string; since: number };

export type Failure = { action: Action; text: string };

const SETTLE = 1200;

export const progress: Record<string, Progress> = $state({});
export const failures: Record<string, Failure> = $state({});
export const finished: Record<string, Action> = $state({});

/** Runs one git action at a time per Project, keeping its progress and failure for whichever view shows that Project. */
export async function perform(projectId: string, action: Action, task: (progress: Progress) => Promise<unknown>, paths: string[] = []): Promise<boolean> {
	if (progress[projectId]) return false;
	progress[projectId] = { action, paths, output: '', since: Date.now() };
	delete failures[projectId];
	try {
		await task(progress[projectId]);
		finished[projectId] = action;
		setTimeout(() => {
			if (finished[projectId] === action) delete finished[projectId];
		}, SETTLE);
		return true;
	} catch (error) {
		failures[projectId] = { action, text: error instanceof Error ? error.message : String(error) };
		return false;
	} finally {
		delete progress[projectId];
	}
}
