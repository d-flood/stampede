import type { Item } from '#lib/device.svelte.ts';
import { SvelteSet } from 'svelte/reactivity';

/** The Chats this Device just opened, which land on their search match once shown. */
export const landings = new SvelteSet<string>();

const word = '[\\p{L}\\p{N}]';
const gap = '[^\\p{L}\\p{N}]+';

/** Lowercases and strips diacritics, recording each folded unit's span in `text`. */
function fold(text: string) {
	let folded = '';
	const starts: number[] = [];
	const ends: number[] = [];
	let at = 0;
	for (const char of text) {
		const plain = char.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase();
		folded += plain;
		for (let unit = 0; unit < plain.length; unit++) {
			starts.push(at);
			ends.push(at + char.length);
		}
		at += char.length;
	}
	return { folded, starts, ends };
}

/** Matches a query's terms as the Host's search does: quoted phrases and words, the last unquoted word as a prefix. */
export function matcher(query: string): RegExp | undefined {
	const terms: { words: string[]; quoted: boolean }[] = [];
	let part = '';
	let quoted = false;
	const push = (phrase: boolean) => {
		const words = fold(part).folded.split(new RegExp(gap, 'u')).filter(Boolean);
		if (words.length) terms.push({ words, quoted: phrase });
		part = '';
	};
	for (const char of query) {
		if (char === '"') {
			push(quoted);
			quoted = !quoted;
		} else if (quoted || new RegExp(word, 'u').test(char)) part += char;
		else push(false);
	}
	push(quoted);
	if (!terms.length) return undefined;
	const last = terms.at(-1)!;
	const sources = terms.map((term) => `(?<!${word})${term.words.join(gap)}${term === last && !term.quoted ? '' : `(?!${word})`}`);
	return new RegExp(sources.join('|'), 'gu');
}

/** Whether a message says any of the terms. */
export function says(item: Item | undefined, pattern: RegExp): boolean {
	if (item?.kind !== 'user' && item?.kind !== 'text') return false;
	return fold(item.text).folded.search(pattern) >= 0;
}

/** Highlights the terms inside `node` without touching its DOM, and returns the marked ranges with a way to clear them. */
export function mark(node: HTMLElement, pattern: RegExp): { ranges: Range[]; clear(): void } {
	const ranges: Range[] = [];
	const walker = document.createTreeWalker(node, NodeFilter.SHOW_TEXT);
	for (let text = walker.nextNode() as Text | null; text; text = walker.nextNode() as Text | null) {
		const { folded, starts, ends } = fold(text.data);
		for (const match of folded.matchAll(pattern)) {
			if (!match[0]) continue;
			const range = new Range();
			range.setStart(text, starts[match.index]);
			range.setEnd(text, ends[match.index + match[0].length - 1]);
			ranges.push(range);
		}
	}
	if (typeof Highlight === 'undefined' || !CSS.highlights) return { ranges, clear() {} };
	const highlight = CSS.highlights.get('landing') ?? new Highlight();
	CSS.highlights.set('landing', highlight);
	for (const range of ranges) highlight.add(range);
	return { ranges, clear: () => ranges.forEach((range) => highlight.delete(range)) };
}
