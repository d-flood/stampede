import { goto } from '$app/navigation';
import { page } from '$app/state';

/** Where a phone is in the Projects → Project overview → Tab or Sidebar section hierarchy. */
export type Level = 'projects' | 'overview' | 'section' | 'tab';

// Waits a task so SvelteKit's router has started before the first history entry.
let queue: Promise<unknown> = new Promise((resolve) => setTimeout(resolve));

export function level(): Level {
	return page.state.level ?? 'tab';
}

export function depth() {
	return page.state.depth ?? 0;
}

function record(level: Level, depth: number, replace = false) {
	return goto(page.url.href, { shallow: true, replace, persistState: true, state: { level, depth } });
}

/** Resolves once SvelteKit applies the entry a traversal reached, or after a second if there was none to reach. */
function popped() {
	const from = page.state;
	return new Promise<void>((resolve) => {
		const timer = setTimeout(done, 1000);
		const stop = $effect.root(() => {
			$effect(() => {
				if (page.state !== from) done();
			});
		});
		function done() {
			clearTimeout(timer);
			resolve();
			queueMicrotask(stop);
		}
	});
}

async function step(target: Level) {
	if (!page.state.level) await record('projects', 0, true);
	const current = level();
	if (target === current) return;
	const from = depth();
	const to = target === 'projects' ? 0 : target === 'overview' ? 1 : current === 'section' || current === 'tab' ? from + 1 : 2;
	if (to < from) {
		// Iframes add their own entries to joint session history, so history.go could step a canvas back instead of this page.
		const entry = 'navigation' in window ? navigation.entries()[(navigation.currentEntry?.index ?? 0) - (from - to)] : undefined;
		if (entry) navigation.traverseTo(entry.key);
		else history.go(to - from);
		return popped();
	}
	if (to === 2 && from === 0) await record('overview', 1);
	await record(target, to);
}

function enqueue(run: () => Promise<unknown>) {
	return (queue = queue.then(run, run));
}

export function seeded() {
	return !!page.state.level;
}

/** Starts at a level unless history already holds one, recording those above it so back goes up from it as from any other. */
export function seed(target: Level) {
	enqueue(async () => page.state.level || step(target));
}

/** Moves to a level; moving up goes back through history, so the system back gesture always goes up a level too. */
export function navigate(target: Level) {
	return enqueue(() => step(target));
}
