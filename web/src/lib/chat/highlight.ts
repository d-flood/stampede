import { language as loadLanguage } from '#lib/editor/languages.ts';
import { highlight } from '#lib/editor/theme.ts';
import { language, type Language } from '@codemirror/language';
import { EditorState } from '@codemirror/state';
import { highlightCode } from '@lezer/highlight';

type Parser = Language['parser'];
type Token = { text: string; classes: string };
type Line = { kind: string; prefix: string; tokens: Token[] };

export async function parser(path: string): Promise<Parser | undefined> {
	const extensions = await loadLanguage(path);
	return EditorState.create({ extensions }).facet(language)?.parser;
}

export function codeLines(code: string, parser?: Parser): Token[][] {
	if (!parser) return code.split('\n').map((text) => [{ text, classes: '' }]);
	const lines: Token[][] = [[]];
	highlightCode(code, parser.parse(code), highlight,
		(text, classes) => lines[lines.length - 1].push({ text, classes }),
		() => lines.push([]));
	return lines;
}

export function diffLines(diff: string, parser?: Parser): Line[] {
	if (!diff) return [];
	const lines: Line[] = [];
	let chunk: string[] = [];
	let hunk = false;

	function flush() {
		if (!chunk.length) return;
		const before = codeLines(chunk.filter((line) => line[0] !== '+').map((line) => line.slice(1)).join('\n'), parser);
		const after = codeLines(chunk.filter((line) => line[0] !== '-').map((line) => line.slice(1)).join('\n'), parser);
		let a = 0;
		let b = 0;
		for (const line of chunk) {
			const prefix = line[0];
			const tokens = prefix === '-' ? before[a] : after[b];
			if (prefix !== '+') a++;
			if (prefix !== '-') b++;
			lines.push({ kind: prefix === '+' ? 'added' : prefix === '-' ? 'removed' : '', prefix, tokens });
		}
		chunk = [];
	}

	for (const line of diff.replace(/\n$/, '').split('\n')) {
		if (!/^[ +\-]/.test(line) || (!hunk && /^(---|\+\+\+) /.test(line))) {
			flush();
			if (line.startsWith('@@')) hunk = true;
			lines.push({ kind: 'hunk', prefix: '', tokens: [{ text: line, classes: '' }] });
		} else chunk.push(line);
	}
	flush();
	return lines;
}
