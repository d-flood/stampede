import { extension, language as loadLanguage } from '#lib/editor/languages.ts';
import { highlight } from '#lib/editor/theme.ts';
import { language, type Language } from '@codemirror/language';
import { EditorState } from '@codemirror/state';
import { highlightCode } from '@lezer/highlight';

type Parser = Language['parser'];
type Token = { text: string; classes: string };
type Line = { kind: string; prefix: string; tokens: Token[] };

const parsers = new Map<string, Promise<Parser | undefined>>();

export function parser(path: string): Promise<Parser | undefined> {
	const key = extension(path);
	let found = parsers.get(key);
	if (!found) {
		found = loadLanguage(path).then((extensions) => EditorState.create({ extensions }).facet(language)?.parser);
		parsers.set(key, found);
	}
	return found;
}

const inline = /\bpython[\d.]*\b[^\n'"|;&]*?(?:-c\s+(?:'([^']*)|"((?:[^"\\]|\\.)*))|<<-?\s*(['"]?)(\w+)\3[^\n]*\n)/dg;

function scripts(code: string): [number, number][] {
	const found: [number, number][] = [];
	const pattern = new RegExp(inline);
	for (let match; (match = pattern.exec(code)); ) {
		let [start, end] = match.indices![1] ?? match.indices![2] ?? [match.index + match[0].length, -1];
		if (end < 0) {
			const terminator = code.slice(start).search(new RegExp(`^\\s*${match[4]}\\s*$`, 'm'));
			end = terminator < 0 ? code.length : start + terminator;
		}
		found.push([start, end]);
		pattern.lastIndex = end;
	}
	return found;
}

function paint(classes: string[], code: string, parser: Parser, offset = 0) {
	let at = offset;
	highlightCode(code, parser.parse(code), highlight,
		(text, style) => {
			classes.fill(style, at, at + text.length);
			at += text.length;
		},
		() => at++);
}

export function codeLines(code: string, parser?: Parser, python?: Parser): Token[][] {
	if (!parser) return code.split('\n').map((text) => [{ text, classes: '' }]);
	const classes = new Array<string>(code.length).fill('');
	paint(classes, code, parser);
	if (python) for (const [start, end] of scripts(code)) paint(classes, code.slice(start, end), python, start);
	const lines: Token[][] = [[]];
	for (let at = 0; at < code.length; at++) {
		const line = lines[lines.length - 1];
		const last = line[line.length - 1];
		if (code[at] === '\n') lines.push([]);
		else if (last?.classes === classes[at]) last.text += code[at];
		else line.push({ text: code[at], classes: classes[at] });
	}
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
