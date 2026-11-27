let started = $state(false);
const waiting: (() => void)[] = [];

export function painted(tabId: string) {
	requestAnimationFrame(() =>
		setTimeout(() => {
			performance.mark(`painted:${tabId}`);
			warmUp();
		})
	);
}

export function warmUp() {
	if (started) return;
	started = true;
	for (const start of waiting.splice(0)) start();
}

export function warm() {
	return started;
}

export function afterPaint(start: () => void) {
	if (started) start();
	else waiting.push(start);
}

export function lazy<T>(load: () => Promise<{ default: T }>): () => T | undefined {
	let component = $state.raw<T>();
	afterPaint(() => load().then((module) => (component = module.default)));
	return () => component;
}

// Returns the module itself once loaded, so {#await} renders it in the same update rather than a task later.
export function cached<T>(load: () => Promise<T>): () => T | Promise<T> {
	let module: T | undefined;
	return () => module ?? load().then((loaded) => (module = loaded));
}
