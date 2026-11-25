import { RangeSetBuilder, countColumn } from '@codemirror/state';
import { Decoration, EditorView, ViewPlugin, type DecorationSet, type ViewUpdate } from '@codemirror/view';

const hangs = new Map<number, Decoration>();

function hang(columns: number) {
	let decoration = hangs.get(columns);
	if (!decoration) hangs.set(columns, (decoration = Decoration.line({ attributes: { style: `--hang: ${columns}ch` } })));
	return decoration;
}

function build(view: EditorView) {
	const builder = new RangeSetBuilder<Decoration>();
	for (const { from, to } of view.visibleRanges) {
		for (let pos = from; pos <= to; ) {
			const line = view.state.doc.lineAt(pos);
			const columns = countColumn(/^[\t ]*/.exec(line.text)![0], view.state.tabSize);
			if (columns) builder.add(line.from, line.from, hang(columns));
			pos = line.to + 1;
		}
	}
	return builder.finish();
}

/** Soft-wraps lines with continuations hanging at each line's own indentation. */
export const indentedWrap = [
	EditorView.lineWrapping,
	ViewPlugin.fromClass(
		class {
			decorations: DecorationSet;
			constructor(view: EditorView) {
				this.decorations = build(view);
			}
			update(update: ViewUpdate) {
				if (update.docChanged || update.viewportChanged) this.decorations = build(update.view);
			}
		},
		{ decorations: (plugin) => plugin.decorations }
	),
	EditorView.theme({ '.cm-line': { paddingLeft: 'calc(var(--space-2) + var(--hang, 0ch))', textIndent: 'calc(-1 * var(--hang, 0ch))' } })
];
