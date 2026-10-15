import type { Project } from '#lib/device.ts';

function segments(path: string): string[] {
	return path.split('/').filter(Boolean);
}

function basename(path: string): string {
	return segments(path).at(-1) ?? '/';
}

export function labels(projects: Project[]): Map<string, string> {
	return new Map(
		projects.map(({ id, path }) => {
			const name = basename(path);
			const collides = projects.some((other) => other.id !== id && basename(other.path) === name);
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
