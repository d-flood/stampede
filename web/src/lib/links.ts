import type { Project } from '#lib/device.svelte.ts';
import type { Cursor } from '#lib/files.svelte.ts';
import { OSC8LinkProvider, type ILink, type Terminal } from 'ghostty-web';

const URLS = /https?:\/\/[\w\-.~:/?#@!$&*+,;=%]+/g;
const TRAILING = /[.,;!?)\]]+$/;
const PATHS = /([./~]?[\w@.-]+(?:\/[\w@.-]+)*\.\w+)(?::(\d+)(?::(\d+))?)?/g;
const FRESH_MS = 10_000;

const checks = new Map<string, { at: number; exists: Promise<boolean> }>();

function exists(projectId: string, path: string): Promise<boolean> {
	const key = `${projectId}:${path}`;
	const checked = checks.get(key);
	if (checked && performance.now() - checked.at < FRESH_MS) return checked.exists;
	const query = new URLSearchParams({ path });
	const found = fetch(`/api/projects/${projectId}/file?${query}`, { method: 'HEAD' }).then(
		(response) => response.ok,
		() => false
	);
	checks.set(key, { at: performance.now(), exists: found });
	return found;
}

function relative(path: string, root: string): string | undefined {
	if (!path.startsWith('/')) return path;
	if (path.startsWith(`${root}/`)) return path.slice(root.length + 1);
}

function browse(url: string) {
	window.open(url, '_blank', 'noopener');
}

export function linkify(view: Terminal, project: () => Project | undefined, onopen: (path: string, cursor?: Cursor) => void) {
	// ghostty-web's built-in providers only follow Ctrl/Cmd-clicks.
	view['linkDetector']?.dispose();
	const hyperlinks = new OSC8LinkProvider(view);

	function link(text: string, range: ILink['range'], open: () => void): ILink {
		return {
			text,
			range,
			activate: () => {
				if (!view.hasSelection()) open();
			}
		};
	}

	function span(y: number, x: number, text: string): ILink['range'] {
		return { start: { x, y }, end: { x: x + text.length - 1, y } };
	}

	async function find(y: number): Promise<ILink[]> {
		const line = view.buffer.active.getLine(y);
		if (!line) return [];
		const osc8 = await new Promise<ILink[] | undefined>((resolve) => hyperlinks.provideLinks(y, resolve));
		const links = (osc8 ?? []).map(({ text, range }) => link(text, range, () => browse(text)));
		let row = '';
		for (let x = 0; x < line.length; x++) {
			const code = line.getCell(x)?.getCodepoint() ?? 0;
			row += code < 32 || code > 0xffff ? ' ' : String.fromCharCode(code);
		}
		const plain = (x: number) => !line.getCell(x)?.getHyperlinkId();
		const urls: [number, number][] = [];
		for (const { 0: match, index } of row.matchAll(URLS)) {
			const url = match.replace(TRAILING, '');
			if (!plain(index)) continue;
			urls.push([index, index + url.length]);
			links.push(link(url, span(y, index, url), () => browse(url)));
		}
		const current = project();
		if (!current) return links;
		const paths = [...row.matchAll(PATHS)].map(async ({ 0: match, 1: written, 2: number, 3: column, index }) => {
			if (!plain(index) || urls.some(([start, end]) => index < end && index + match.length > start)) return;
			const path = relative(written, current.path);
			if (!path || !(await exists(current.id, path))) return;
			const cursor = number ? { line: Number(number), column: Number(column ?? 1) } : undefined;
			return link(match, span(y, index, match), () => onopen(path, cursor));
		});
		for (const found of await Promise.all(paths)) if (found) links.push(found);
		return links;
	}

	view.registerLinkProvider({ provideLinks: (y, callback) => find(y).then((links) => callback(links.length ? links : undefined)) });

	const element = view.element;
	let pressed: PointerEvent | undefined;
	element?.addEventListener('pointerdown', (event) => (pressed = event));
	// ghostty-web cancels touchend, so a tap never becomes the click its links follow.
	element?.addEventListener('pointerup', ({ pointerType, clientX, clientY }) => {
		if (pointerType !== 'touch' || !pressed || Math.hypot(clientX - pressed.clientX, clientY - pressed.clientY) > 8) return;
		element.dispatchEvent(new MouseEvent('click', { clientX, clientY }));
	});
}
