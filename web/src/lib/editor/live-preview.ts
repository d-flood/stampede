import type { markdown } from '@codemirror/lang-markdown';
import { ensureSyntaxTree, syntaxTree } from '@codemirror/language';
import { StateEffect, StateField, type EditorState, type Extension, type Range } from '@codemirror/state';
import { Decoration, EditorView, type DecorationSet } from '@codemirror/view';
import { tags } from '@lezer/highlight';
import {
	ChipWidget,
	ImageWidget,
	imageSource,
	MarkerWidget,
	TableWidget,
	TaskWidget,
	type ImageSource,
	type InlineNode,
	type TableCell,
	type TableModel
} from './preview-widgets.ts';

type SyntaxNode = ReturnType<typeof syntaxTree>['topNode'];
type MarkdownConfig = Exclude<NonNullable<NonNullable<Parameters<typeof markdown>[0]>['extensions']>, readonly unknown[]>;

const OPEN_BRACKET = 91;
const CARET = 94;
const DEFINITION = /^\[\^([^\s\]]+)\]:[ \t]*/;

export const footnotes: MarkdownConfig = {
	defineNodes: [
		{ name: 'FootnoteDefinition', block: true },
		{ name: 'FootnoteReference' },
		{ name: 'FootnoteMark', style: tags.processingInstruction },
		{ name: 'FootnoteLabel', style: tags.labelName }
	],
	parseInline: [
		{
			name: 'FootnoteReference',
			before: 'Link',
			parse(cx, next, pos) {
				if (next !== OPEN_BRACKET || cx.char(pos + 1) !== CARET) return -1;
				const label = /^[^\s\]]+\]/.exec(cx.slice(pos + 2, cx.end));
				if (!label) return -1;
				const end = pos + 2 + label[0].length;
				return cx.addElement(
					cx.elt('FootnoteReference', pos, end, [
						cx.elt('FootnoteMark', pos, pos + 2),
						cx.elt('FootnoteLabel', pos + 2, end - 1),
						cx.elt('FootnoteMark', end - 1, end)
					])
				);
			}
		}
	],
	parseBlock: [
		{
			name: 'FootnoteDefinition',
			before: 'LinkReference',
			leaf: (_cx, leaf) =>
				DEFINITION.test(leaf.content)
					? {
							nextLine: () => false,
							finish(cx, leaf) {
								const head = DEFINITION.exec(leaf.content);
								if (!head) return false;
								const labelEnd = leaf.start + 2 + head[1].length;
								const bodyStart = leaf.start + head[0].length;
								cx.addLeafElement(
									leaf,
									cx.elt('FootnoteDefinition', leaf.start, leaf.start + leaf.content.length, [
										cx.elt('FootnoteMark', leaf.start, leaf.start + 2),
										cx.elt('FootnoteLabel', leaf.start + 2, labelEnd),
										cx.elt('FootnoteMark', labelEnd, labelEnd + 2),
										...cx.parser.parseInline(leaf.content.slice(head[0].length), bodyStart)
									])
								);
								return true;
							}
						}
					: null,
			endLeaf: (_cx, line) => DEFINITION.test(line.text.slice(line.pos))
		}
	]
};

const setFocused = StateEffect.define<boolean>();
const setFrontMatterOpen = StateEffect.define<boolean>();

const focusedField = StateField.define<boolean>({
	create: () => false,
	update: (value, tr) => tr.effects.reduce((next, effect) => (effect.is(setFocused) ? effect.value : next), value)
});

const frontMatterOpenField = StateField.define<boolean>({
	create: () => false,
	update: (value, tr) => tr.effects.reduce((next, effect) => (effect.is(setFrontMatterOpen) ? effect.value : next), value)
});

// An editor focused on open has its caret at the start, which must not open the front matter.
const movedField = StateField.define<boolean>({
	create: () => false,
	update: (value, tr) => value || !!tr.selection || tr.docChanged
});

const hidden = Decoration.replace({});
const HEADING = /^(?:ATX|Setext)Heading([1-6])$/;
const BULLET = '•';

function touchedLines(state: EditorState) {
	const lines = new Set<number>();
	if (!state.field(focusedField)) return lines;
	for (const range of state.selection.ranges) {
		const last = state.doc.lineAt(range.to).number;
		for (let line = state.doc.lineAt(range.from).number; line <= last; line++) lines.add(line);
	}
	return lines;
}

function imageParts(state: EditorState, image: SyntaxNode, linkLabels: Map<string, string>) {
	const url = image.getChild('URL');
	const marks = image.getChildren('LinkMark');
	if (marks.length < 2) return null;
	const alt = state.sliceDoc(marks[0].to, marks[1].from);
	const src = url ? state.sliceDoc(url.from, url.to) : marks.length < 4 ? linkLabels.get(referenceLabel(state, image, alt)) : undefined;
	if (!src) return null;
	return { src: src.startsWith('<') && src.endsWith('>') ? src.slice(1, -1) : src, alt };
}

/** The label a reference names, `text` itself for collapsed and shortcut references. */
function referenceLabel(state: EditorState, node: SyntaxNode, text: string) {
	const reference = node.getChild('LinkLabel');
	return normalizeLabel(reference && reference.to - reference.from > 2 ? state.sliceDoc(reference.from + 1, reference.to - 1) : text);
}

function normalizeLabel(label: string) {
	return label.trim().replace(/\s+/g, ' ').toLowerCase();
}

function scanDefinitions(state: EditorState, tree: ReturnType<typeof syntaxTree>) {
	const linkLabels = new Map<string, string>();
	const footnoteNumbers = new Map<string, number>();
	tree.iterate({
		enter(node) {
			if (node.name === 'LinkReference') {
				const [label, url] = [node.node.getChild('LinkLabel'), node.node.getChild('URL')];
				const key = label && normalizeLabel(state.sliceDoc(label.from + 1, label.to - 1));
				if (key && !linkLabels.has(key)) linkLabels.set(key, url ? state.sliceDoc(url.from, url.to) : '');
			} else if (node.name === 'FootnoteReference') {
				const label = state.sliceDoc(node.from + 2, node.to - 1);
				if (!footnoteNumbers.has(label)) footnoteNumbers.set(label, footnoteNumbers.size + 1);
			} else if (node.name === 'FencedCode' || node.name === 'CodeBlock' || node.name === 'Frontmatter') {
				return false;
			}
		}
	});
	return { linkLabels, footnoteNumbers };
}

function isRealLink(state: EditorState, link: SyntaxNode, linkLabels: Map<string, string>) {
	const marks = link.getChildren('LinkMark');
	if (link.getChild('URL') || marks.length >= 4) return true;
	if (marks.length < 2) return false;
	return linkLabels.has(referenceLabel(state, link, state.sliceDoc(marks[0].to, marks[1].from)));
}

function skipSpaces(state: EditorState, pos: number) {
	const end = state.doc.lineAt(pos).to;
	while (pos < end && state.sliceDoc(pos, pos + 1) === ' ') pos++;
	return pos;
}

function skipOneSpace(state: EditorState, pos: number) {
	return state.sliceDoc(pos, pos + 1) === ' ' ? pos + 1 : pos;
}

function indentStart(state: EditorState, pos: number) {
	const start = state.doc.lineAt(pos).from;
	while (pos > start && /[ \t]/.test(state.sliceDoc(pos - 1, pos))) pos--;
	return pos;
}

function toggleTask(view: EditorView, pos: number) {
	const checked = /x/i.test(view.state.sliceDoc(pos + 1, pos + 2));
	view.dispatch({ changes: { from: pos + 1, to: pos + 2, insert: checked ? ' ' : 'x' }, userEvent: 'input.task' });
}

function isFrontMatterTouched(state: EditorState, node: SyntaxNode) {
	if (!state.field(movedField)) return false;
	const touched = touchedLines(state);
	const last = state.doc.lineAt(node.to - 1).number;
	for (let line = state.doc.lineAt(node.from).number; line <= last; line++) {
		if (touched.has(line)) return true;
	}
	return false;
}

function toggleFrontMatter(view: EditorView) {
	const { state } = view;
	const node = syntaxTree(state).topNode.getChild('Frontmatter');
	if (!node) return;
	const lastLine = state.doc.lineAt(node.to - 1).number;
	if (!state.field(frontMatterOpenField) && !isFrontMatterTouched(state, node)) {
		const firstLine = state.doc.lineAt(node.from).number;
		const editable = state.doc.line(Math.min(firstLine + 1, lastLine));
		view.dispatch({ effects: setFrontMatterOpen.of(true), selection: { anchor: editable.from } });
		view.focus();
		return;
	}
	const after = state.doc.lines > lastLine ? state.doc.line(lastLine + 1).from : state.doc.length;
	view.dispatch({
		effects: setFrontMatterOpen.of(false),
		selection: isFrontMatterTouched(state, node) ? { anchor: after } : undefined
	});
	if (state.doc.lines === lastLine) view.contentDOM.blur();
}

function inlineChildren(
	state: EditorState,
	parent: SyntaxNode,
	from: number,
	to: number,
	footnoteNumbers: Map<string, number>,
	linkLabels: Map<string, string>
): InlineNode[] {
	const nodes: InlineNode[] = [];
	let pos = from;
	const text = (end: number) => {
		if (end > pos) nodes.push({ kind: 'text', text: state.sliceDoc(pos, end) });
	};
	const inner = (node: SyntaxNode, mark: string) => {
		const marks = node.getChildren(mark);
		const start = marks.length ? marks[0].to : node.from;
		const end = marks.length > 1 ? marks[marks.length - 1].from : node.to;
		return inlineChildren(state, node, start, end, footnoteNumbers, linkLabels);
	};
	const kinds = { Emphasis: 'em', StrongEmphasis: 'strong', Strikethrough: 's' } as const;

	for (let child = parent.firstChild; child; child = child.nextSibling) {
		if (child.to <= from || child.from >= to) continue;
		const { name } = child;
		const parts = name === 'Image' ? imageParts(state, child, linkLabels) : null;
		if (name in kinds) {
			text(child.from);
			const mark = name === 'Strikethrough' ? 'StrikethroughMark' : 'EmphasisMark';
			nodes.push({ kind: kinds[name as keyof typeof kinds], children: inner(child, mark) });
		} else if (name === 'InlineCode') {
			text(child.from);
			nodes.push({ kind: 'code', children: inner(child, 'CodeMark') });
		} else if (name === 'Link' && isRealLink(state, child, linkLabels)) {
			text(child.from);
			const marks = child.getChildren('LinkMark');
			const start = marks[0]?.to ?? child.from;
			const end = marks[1]?.from ?? child.to;
			nodes.push({ kind: 'link', children: inlineChildren(state, child, start, end, footnoteNumbers, linkLabels) });
		} else if (parts) {
			text(child.from);
			nodes.push({ kind: 'image', ...parts });
		} else if (name === 'URL' || name === 'Autolink') {
			text(child.from);
			const url = name === 'Autolink' ? child.getChild('URL') : child;
			const value = url ? state.sliceDoc(url.from, url.to) : state.sliceDoc(child.from, child.to);
			nodes.push({ kind: 'link', children: [{ kind: 'text', text: value }] });
		} else if (name === 'FootnoteReference') {
			text(child.from);
			const label = state.sliceDoc(child.from + 2, child.to - 1);
			nodes.push({ kind: 'sup', text: String(footnoteNumbers.get(label) ?? label) });
		} else if (name === 'Escape') {
			text(child.from);
			nodes.push({ kind: 'text', text: state.sliceDoc(child.from + 1, child.to) });
		} else {
			continue;
		}
		pos = child.to;
	}
	text(to);
	return nodes;
}

function tableCells(row: SyntaxNode, makeCell: (cell: SyntaxNode) => TableCell) {
	const cells: (TableCell | null)[] = [];
	let first = true;
	let sawCell = false;
	for (let child = row.firstChild; child; child = child.nextSibling) {
		if (child.name === 'TableDelimiter') {
			if (!first && !sawCell) cells.push(null);
			sawCell = false;
		} else if (child.name === 'TableCell') {
			cells.push(makeCell(child));
			sawCell = true;
		}
		first = false;
	}
	return cells;
}

function tableModel(
	state: EditorState,
	table: SyntaxNode,
	from: number,
	footnoteNumbers: Map<string, number>,
	linkLabels: Map<string, string>
): TableModel | null {
	const headRow = table.getChild('TableHeader');
	if (!headRow) return null;
	const makeCell = (cell: SyntaxNode): TableCell => ({
		offset: cell.from - from,
		content: inlineChildren(state, cell, cell.from, cell.to, footnoteNumbers, linkLabels)
	});
	const head = tableCells(headRow, makeCell);
	const fit = (cells: (TableCell | null)[]) => head.map((_, index) => cells[index] ?? null);
	const delimiter = table.getChild('TableDelimiter');
	const aligns = delimiter
		? state
				.sliceDoc(delimiter.from, delimiter.to)
				.replace(/^\s*\|/, '')
				.replace(/\|\s*$/, '')
				.split('|')
				.map((spec) => {
					const trimmed = spec.trim();
					const left = trimmed.startsWith(':');
					const right = trimmed.endsWith(':');
					return left && right ? 'center' : right ? 'right' : left ? 'left' : null;
				})
		: [];
	return { aligns, head, rows: table.getChildren('TableRow').map((row) => fit(tableCells(row, makeCell))) };
}

function buildDecorations(state: EditorState, tree = syntaxTree(state)): DecorationSet {
	const { doc } = state;
	const active = touchedLines(state);
	const { linkLabels, footnoteNumbers } = scanDefinitions(state, tree);
	const ranges: Range<Decoration>[] = [];
	const blockRanges: { from: number; to: number }[] = [];

	const quoteDepth = new Map<number, number>();
	const listDepth = new Map<number, number>();
	const markerLines = new Set<number>();
	const codeLines = new Set<number>();
	const lineClasses = new Map<number, string[]>();
	let quoteLevel = 0;
	let listLevel = 0;
	let lastFootnoteLine = -2;

	const isActive = (pos: number) => active.has(doc.lineAt(pos).number);
	const eachLine = (from: number, to: number, visit: (line: number) => void) => {
		const last = doc.lineAt(to).number;
		for (let line = doc.lineAt(from).number; line <= last; line++) visit(line);
	};
	const addLineClass = (line: number, name: string) => {
		const classes = lineClasses.get(line);
		if (classes) classes.push(name);
		else lineClasses.set(line, [name]);
	};
	const hide = (from: number, to: number) => {
		if (to > from && !isActive(from)) ranges.push(hidden.range(from, to));
	};

	tree.iterate({
		enter(ref) {
			const { name, from, to } = ref;

			if (name === 'Frontmatter') {
				if (ref.node.getChildren('DashLine').length < 2) return false;
				const lines = { from: doc.lineAt(from).from, to: doc.lineAt(to - 1).to };
				if (state.field(frontMatterOpenField) || isFrontMatterTouched(state, ref.node)) {
					ranges.push(Decoration.widget({ widget: new ChipWidget(true, toggleFrontMatter), block: true, side: -1 }).range(lines.from));
					eachLine(lines.from, lines.to, (line) => addLineClass(line, 'cm-lp-frontmatter'));
				} else {
					ranges.push(Decoration.replace({ widget: new ChipWidget(false, toggleFrontMatter), block: true }).range(lines.from, lines.to));
					blockRanges.push(lines);
				}
				return false;
			}

			const heading = HEADING.exec(name);
			if (heading) {
				const lastTextLine = name.startsWith('Setext') ? doc.lineAt(to).number - 1 : doc.lineAt(to).number;
				for (let line = doc.lineAt(from).number; line <= lastTextLine; line++) addLineClass(line, `cm-lp-h${heading[1]}`);
				return;
			}

			switch (name) {
				case 'HeaderMark':
					if (ref.node.parent?.from === from) hide(from, skipSpaces(state, to));
					else hide(indentStart(state, from), to);
					return;
				case 'EmphasisMark':
				case 'StrikethroughMark':
				case 'CodeMark':
					if (ref.node.parent?.name === 'InlineCode' || name !== 'CodeMark') hide(from, to);
					return;
				case 'InlineCode':
					ranges.push(Decoration.mark({ class: 'cm-lp-inline-code' }).range(from, to));
					return;
				case 'Escape':
					hide(from, from + 1);
					return false;
				case 'Image': {
					const parts = imageParts(state, ref.node, linkLabels);
					if (parts && !isActive(from) && !isActive(to)) {
						ranges.push(Decoration.replace({ widget: new ImageWidget(parts.src, parts.alt) }).range(from, to));
					}
					return false;
				}
				case 'HTMLBlock':
				case 'HTMLTag':
				case 'CommentBlock':
				case 'ProcessingInstructionBlock':
				case 'LinkReference':
					return false;
				case 'Link': {
					if (!isRealLink(state, ref.node, linkLabels)) {
						ranges.push(Decoration.mark({ class: 'cm-lp-unlinked' }).range(from, to));
						return false;
					}
					const marks = ref.node.getChildren('LinkMark');
					if (marks.length < 2) return false;
					hide(marks[0].from, marks[0].to);
					hide(marks[1].from, to);
					ranges.push(Decoration.mark({ class: 'cm-lp-link' }).range(marks[0].to, marks[1].from));
					return;
				}
				case 'Autolink': {
					const marks = ref.node.getChildren('LinkMark');
					if (marks.length === 2) {
						hide(marks[0].from, marks[0].to);
						hide(marks[1].from, marks[1].to);
					}
					return;
				}
				case 'URL':
					if (ref.node.parent?.name !== 'Link') ranges.push(Decoration.mark({ class: 'cm-lp-link' }).range(from, to));
					return;
				case 'Blockquote':
					quoteLevel++;
					eachLine(from, to, (line) => quoteDepth.set(line, quoteLevel));
					return;
				case 'QuoteMark':
					hide(from, skipOneSpace(state, to));
					return;
				case 'BulletList':
				case 'OrderedList':
					listLevel++;
					return;
				case 'ListItem':
					eachLine(from, to, (line) => listDepth.set(line, listLevel));
					return;
				case 'ListMark': {
					const lineNumber = doc.lineAt(from).number;
					markerLines.add(lineNumber);
					if (active.has(lineNumber)) return;
					const start = indentStart(state, from);
					const end = skipOneSpace(state, to);
					if (ref.node.nextSibling?.name === 'Task') {
						ranges.push(hidden.range(start, end));
					} else {
						const ordered = ref.node.parent?.parent?.name === 'OrderedList';
						const widget = new MarkerWidget(ordered ? state.sliceDoc(from, to) : BULLET, ordered ? 'cm-lp-ordinal' : 'cm-lp-bullet');
						ranges.push(Decoration.replace({ widget }).range(start, end));
					}
					return;
				}
				case 'TaskMarker': {
					const checked = /x/i.test(state.sliceDoc(from + 1, from + 2));
					const lineNumber = doc.lineAt(from).number;
					if (checked) addLineClass(lineNumber, 'cm-lp-done');
					addLineClass(lineNumber, 'cm-lp-task');
					if (!active.has(lineNumber)) {
						ranges.push(Decoration.replace({ widget: new TaskWidget(checked, toggleTask) }).range(from, skipOneSpace(state, to)));
					}
					return;
				}
				case 'HorizontalRule': {
					const lineNumber = doc.lineAt(from).number;
					if (!active.has(lineNumber)) {
						addLineClass(lineNumber, 'cm-lp-hr');
						ranges.push(hidden.range(from, to));
					}
					return false;
				}
				case 'FencedCode':
				case 'CodeBlock': {
					eachLine(from, to, (line) => {
						codeLines.add(line);
						addLineClass(line, 'cm-lp-code');
					});
					for (const mark of ref.node.getChildren('QuoteMark')) hide(mark.from, skipOneSpace(state, mark.to));
					if (name === 'CodeBlock') return false;
					for (const fence of ref.node.getChildren('CodeMark')) {
						const line = doc.lineAt(fence.from);
						if (active.has(line.number)) {
							addLineClass(line.number, 'cm-lp-fence-raw');
						} else {
							addLineClass(line.number, 'cm-lp-fence');
							ranges.push(hidden.range(indentStart(state, fence.from), line.to));
						}
					}
					return false;
				}
				case 'Table': {
					const lines = { from: doc.lineAt(from).from, to: doc.lineAt(to).to };
					let touched = false;
					eachLine(lines.from, lines.to, (line) => {
						touched ||= active.has(line);
					});
					if (touched) {
						eachLine(lines.from, lines.to, (line) => addLineClass(line, 'cm-lp-table-raw'));
						return false;
					}
					const model = tableModel(state, ref.node, lines.from, footnoteNumbers, linkLabels);
					if (!model) return false;
					ranges.push(
						Decoration.replace({ widget: new TableWidget(state.sliceDoc(lines.from, lines.to), model), block: true }).range(lines.from, lines.to)
					);
					blockRanges.push(lines);
					return false;
				}
				case 'FootnoteReference': {
					if (!isActive(from)) {
						const label = state.sliceDoc(from + 2, to - 1);
						const widget = new MarkerWidget(String(footnoteNumbers.get(label) ?? label), 'cm-lp-fnref');
						ranges.push(Decoration.replace({ widget }).range(from, to));
					}
					return false;
				}
				case 'FootnoteDefinition': {
					const firstLine = doc.lineAt(from).number;
					eachLine(from, to, (line) => {
						addLineClass(line, 'cm-lp-fndef');
						if (line === firstLine && firstLine > lastFootnoteLine + 2) addLineClass(line, 'cm-lp-fndef-first');
					});
					lastFootnoteLine = doc.lineAt(to).number;
					const marks = ref.node.getChildren('FootnoteMark');
					if (!isActive(from) && marks.length === 2) {
						const label = state.sliceDoc(marks[0].to, marks[1].from);
						const widget = new MarkerWidget(String(footnoteNumbers.get(label) ?? label), 'cm-lp-fnnum');
						ranges.push(Decoration.replace({ widget }).range(from, skipSpaces(state, marks[1].to)));
					}
					return;
				}
			}
		},
		leave(ref) {
			if (ref.name === 'Blockquote') quoteLevel--;
			if (ref.name === 'BulletList' || ref.name === 'OrderedList') listLevel--;
		}
	});

	for (const line of new Set([...quoteDepth.keys(), ...listDepth.keys()])) {
		const quote = quoteDepth.get(line) ?? 0;
		const list = listDepth.get(line) ?? 0;
		if (quote) {
			addLineClass(line, 'cm-lp-quote');
			if (!quoteDepth.has(line - 1)) addLineClass(line, 'cm-lp-quote-first');
			if (!quoteDepth.has(line + 1)) addLineClass(line, 'cm-lp-quote-last');
		}
		if (list) addLineClass(line, markerLines.has(line) ? 'cm-lp-li' : 'cm-lp-li-rest');
		if (list && !markerLines.has(line) && !codeLines.has(line) && !active.has(line)) {
			const { from, text } = doc.line(line);
			const indent = /^[ \t]+/.exec(text);
			if (indent) ranges.push(hidden.range(from, from + indent[0].length));
		}
	}

	for (const [line, classes] of lineClasses) {
		const quote = quoteDepth.get(line) ?? 0;
		const list = listDepth.get(line) ?? 0;
		ranges.push(
			Decoration.line({
				class: classes.join(' '),
				attributes: quote || list ? { style: `--quote:${quote};--list:${list}` } : undefined
			}).range(doc.line(line).from)
		);
	}

	return Decoration.set(
		ranges.filter(({ from, to }) => !blockRanges.some((block) => from >= block.from && to <= block.to && !(from === block.from && to === block.to))),
		true
	);
}

const previewField = StateField.define<DecorationSet>({
	create(state) {
		return buildDecorations(state, ensureSyntaxTree(state, state.doc.length, 200) ?? syntaxTree(state));
	},
	update(value, tr) {
		const relevant =
			tr.docChanged ||
			tr.selection ||
			tr.effects.some((effect) => effect.is(setFocused) || effect.is(setFrontMatterOpen)) ||
			syntaxTree(tr.state) !== syntaxTree(tr.startState);
		return relevant ? buildDecorations(tr.state) : value;
	},
	provide: (field) => EditorView.decorations.from(field)
});

const indent = (extra = '0px') => `calc(var(--space-2) + var(--quote, 0) * 0.85em + var(--list, 0) * 1.5em + ${extra})`;

const styles = EditorView.theme({
	'&.cm-live-preview': { fontSize: 'var(--font-size-chat)' },
	'&.cm-live-preview .cm-scroller': { fontFamily: 'var(--font-chat)', lineHeight: '1.6' },

	'.cm-lp-h1, .cm-lp-h2, .cm-lp-h3': { lineHeight: '1.25' },
	'.cm-lp-h1': { paddingTop: 'var(--space-4)', fontSize: '1.6em' },
	'.cm-lp-h2': { paddingTop: 'var(--space-3)', fontSize: '1.35em' },
	'.cm-lp-h3': { paddingTop: 'var(--space-2)', fontSize: '1.15em' },

	'.cm-lp-link': { color: 'var(--accent)', textDecoration: 'underline', textUnderlineOffset: '0.14em' },
	'.cm-lp-unlinked, .cm-lp-unlinked *': { color: 'var(--color-fg)', textDecoration: 'none' },
	'.cm-lp-inline-code': {
		padding: '1px var(--space-1)',
		borderRadius: 'var(--radius-md)',
		background: 'var(--surface-2)',
		fontFamily: 'var(--font-mono)',
		fontSize: '0.875em'
	},
	'.cm-lp-image': { maxWidth: '100%', height: 'auto', verticalAlign: 'bottom' },
	'.cm-lp-image-alt': { color: 'var(--muted-fg)', fontStyle: 'italic' },

	'.cm-lp-quote': {
		paddingLeft: indent(),
		color: 'var(--muted-fg)',
		backgroundImage: 'repeating-linear-gradient(to right, var(--border) 0 var(--border-width), transparent var(--border-width) 0.85em)',
		backgroundPosition: 'var(--space-2) 0',
		backgroundSize: 'calc(var(--quote, 1) * 0.85em) 100%',
		backgroundRepeat: 'no-repeat'
	},
	'.cm-lp-li, .cm-lp-li-rest': { paddingLeft: indent() },
	'.cm-lp-li': { textIndent: '-1.5em' },
	'.cm-lp-bullet, .cm-lp-ordinal, .cm-lp-task-box': { display: 'inline-block', width: '1.5em', textIndent: '0' },
	'.cm-lp-bullet': { color: 'var(--muted-fg)' },
	'.cm-lp-ordinal': { color: 'var(--muted-fg)', fontVariantNumeric: 'lining-nums tabular-nums' },
	'.cm-lp-task-box': {
		position: 'relative',
		isolation: 'isolate',
		display: 'inline-flex',
		alignItems: 'center',
		height: '1em',
		verticalAlign: '-0.14em',
		cursor: 'pointer'
	},
	'.cm-lp-task-box::after': {
		content: '""',
		position: 'absolute',
		top: '50%',
		left: 'calc(1.5em - var(--target-min))',
		width: 'var(--target-min)',
		height: 'var(--target-min)',
		zIndex: '-1',
		transform: 'translateY(-50%)'
	},
	'.cm-lp-task-box input': {
		appearance: 'none',
		display: 'block',
		flex: 'none',
		width: '0.75em',
		height: '0.75em',
		margin: '0',
		border: '1.5px solid var(--accent)',
		borderRadius: '3px',
		background: 'transparent',
		cursor: 'pointer'
	},
	'.cm-lp-task-box input:checked': { background: 'var(--accent)' },
	'.cm-lp-done': { color: 'var(--muted-fg)' },

	'.cm-lp-hr': { position: 'relative' },
	'.cm-lp-hr::after': {
		content: '""',
		position: 'absolute',
		top: '50%',
		right: 'var(--space-2)',
		left: 'var(--space-2)',
		borderTop: 'var(--border-width) solid var(--border-strong)'
	},

	'.cm-lp-code, .cm-lp-frontmatter, .cm-lp-table-raw': { fontFamily: 'var(--font-mono)', fontSize: 'var(--font-size-code)' },
	'.cm-lp-code': { paddingInline: 'var(--space-3)', background: 'var(--surface-1)' },
	'.cm-lp-li-rest.cm-lp-code, .cm-lp-quote.cm-lp-code': { paddingLeft: indent('var(--space-3)') },
	'.cm-lp-fence-raw': { color: 'var(--muted-fg)' },
	'.cm-lp-frontmatter': { paddingInline: 'var(--space-3)', background: 'var(--surface-1)', color: 'var(--muted-fg)' },
	'.cm-lp-chip-row': { padding: '0 0 var(--space-3)' },
	'.cm-lp-chip': {
		display: 'inline-flex',
		alignItems: 'center',
		gap: 'var(--space-2)',
		minHeight: 'var(--target-min)',
		padding: '0 var(--space-2)',
		border: '0',
		borderRadius: 'var(--radius-sm)',
		background: 'transparent',
		color: 'var(--muted-fg)',
		fontFamily: 'var(--font-ui)',
		fontSize: 'var(--font-size-sm)',
		cursor: 'pointer'
	},
	'.cm-lp-chip:hover': { background: 'var(--hover)' },
	'.cm-lp-chip::before': {
		content: '""',
		width: '0.45em',
		height: '0.6em',
		background: 'currentColor',
		clipPath: 'polygon(0 0, 100% 50%, 0 100%)'
	},
	'.cm-lp-chip[aria-expanded="true"]::before': { rotate: '90deg' },

	'.cm-lp-table-wrap': { padding: 'var(--space-3) var(--space-2)', overflowX: 'auto', cursor: 'text' },
	'.cm-lp-table': {
		borderCollapse: 'collapse',
		borderRadius: 'var(--radius-lg)',
		background: 'var(--surface-1)',
		boxShadow: 'var(--shadow-lift)',
		fontVariantNumeric: 'lining-nums tabular-nums',
		lineHeight: '1.35'
	},
	'.cm-lp-table th, .cm-lp-table td': {
		padding: 'var(--space-2) var(--space-3)',
		borderBottom: 'var(--border-width) solid var(--border)',
		textAlign: 'start',
		verticalAlign: 'top',
		whiteSpace: 'normal'
	},
	'.cm-lp-table th': { background: 'var(--surface-2)', fontWeight: '600' },
	'.cm-lp-table tbody tr:last-child td': { borderBottom: 'none' },

	'.cm-lp-fnref': { verticalAlign: 'super', color: 'var(--accent)', fontSize: '0.7em', lineHeight: '0' },
	'.cm-lp-fndef': { color: 'var(--muted-fg)', fontSize: '0.9em' },
	'.cm-lp-fndef-first': { position: 'relative', paddingTop: 'var(--space-3)' },
	'.cm-lp-fndef-first::before': {
		content: '""',
		position: 'absolute',
		top: '0',
		left: 'var(--space-2)',
		width: '5rem',
		borderTop: 'var(--border-width) solid var(--border-strong)'
	},
	'.cm-lp-fnnum': { marginRight: '0.4em', color: 'var(--accent)', fontSize: '0.8em', verticalAlign: 'super', lineHeight: '0' },

	'@media (pointer: coarse)': {
		'.cm-lp-task': { paddingBlock: 'var(--space-2)' },
		'.cm-lp-task-box input': { width: '1.1em', height: '1.1em' }
	}
});

export function livePreview(options: { image?: ImageSource } = {}): Extension {
	return [
		focusedField,
		frontMatterOpenField,
		movedField,
		// focusChangeEffect drops its transaction when another dispatch lands first, so track DOM focus directly.
		EditorView.domEventHandlers({
			focus: (_event, view) => view.dispatch({ effects: setFocused.of(true) }),
			blur: (_event, view) => view.dispatch({ effects: setFocused.of(false) })
		}),
		previewField,
		imageSource.of(options.image ?? (() => null)),
		EditorView.editorAttributes.of({ class: 'cm-live-preview' }),
		styles
	];
}
