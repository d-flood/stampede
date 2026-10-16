import { Chunk } from '@codemirror/merge';
import { Prec, StateEffect, StateField, type ChangeDesc, type Text } from '@codemirror/state';
import {
	Decoration,
	EditorView,
	GutterMarker,
	ViewPlugin,
	WidgetType,
	gutter,
	keymap,
	type ViewUpdate
} from '@codemirror/view';

type Kind = 'added' | 'modified' | 'deleted';

type Changes = { baseline: Text | null; chunks: readonly Chunk[]; open: number | null };

const DEBOUNCE = 250;
const diffConfig = { scanLimit: 500 };

export const setBaseline = StateEffect.define<Text | null>();
const recompute = StateEffect.define<null>();
const toggle = StateEffect.define<number | null>();

function kind(chunk: Chunk): Kind {
	if (chunk.fromA === chunk.toA) return 'added';
	return chunk.fromB === chunk.toB ? 'deleted' : 'modified';
}

function covers(chunk: Chunk, pos: number) {
	return chunk.fromB <= pos && pos <= chunk.endB;
}

function map(chunk: Chunk, changes: ChangeDesc) {
	const pos = (at: number) =>
		at > changes.length ? changes.mapPos(changes.length) + at - changes.length : changes.mapPos(at);
	return new Chunk(chunk.changes, chunk.fromA, chunk.toA, pos(chunk.fromB), pos(chunk.toB), chunk.precise);
}

function original(baseline: Text, chunk: Chunk) {
	return baseline.sliceString(chunk.fromA, Math.max(chunk.fromA, chunk.toA - 1));
}

const changes = StateField.define<Changes>({
	create: () => ({ baseline: null, chunks: [], open: null }),
	update(value, tr) {
		const effects = tr.effects.filter((effect) => effect.is(setBaseline) || effect.is(recompute) || effect.is(toggle));
		if (!effects.length && (!tr.docChanged || !value.chunks.length)) return value;
		let { baseline, chunks, open } = value;
		const at = (pos: number | null) => (pos === null ? undefined : chunks.find((chunk) => covers(chunk, pos)));
		if (tr.docChanged) {
			chunks = chunks.map((chunk) => map(chunk, tr.changes));
			if (open !== null) open = tr.changes.mapPos(open);
		}
		for (const effect of effects) {
			if (effect.is(setBaseline)) baseline = effect.value;
			if (effect.is(toggle)) open = effect.value === null || at(open) === at(effect.value) ? null : effect.value;
			else chunks = baseline ? Chunk.build(baseline, tr.newDoc, diffConfig) : [];
		}
		return { baseline, chunks, open: at(open)?.fromB ?? null };
	},
	provide: (field) =>
		EditorView.decorations.from(field, ({ baseline, chunks, open }) => {
			const chunk = chunks.find((chunk) => chunk.fromB === open);
			if (!baseline || !chunk) return Decoration.none;
			const deleted = kind(chunk) === 'deleted';
			const widget = new Original(kind(chunk) === 'added' ? null : original(baseline, chunk));
			return Decoration.set(
				Decoration.widget({ widget, block: true, side: deleted ? -1 : 1 }).range(deleted ? chunk.fromB : chunk.endB)
			);
		})
});

function revert(view: EditorView) {
	const { state } = view;
	const { baseline, open } = state.field(changes);
	if (!baseline || open === null) return;
	const chunk = Chunk.build(baseline, state.doc, diffConfig).find((chunk) => covers(chunk, open));
	if (!chunk) return;
	let insert = original(baseline, chunk);
	if (chunk.fromA !== chunk.toA && chunk.toB <= state.doc.length) insert += state.lineBreak;
	view.dispatch({
		changes: { from: chunk.fromB, to: Math.min(state.doc.length, chunk.toB), insert },
		effects: recompute.of(null),
		userEvent: 'revert'
	});
}

class Original extends WidgetType {
	constructor(readonly lines: string | null) {
		super();
	}

	eq(other: Original) {
		return other.lines === this.lines;
	}

	toDOM(view: EditorView) {
		const dom = document.createElement('div');
		dom.className = 'cm-changeOriginal';
		if (this.lines !== null) {
			const lines = dom.appendChild(document.createElement('pre'));
			lines.textContent = this.lines;
		}
		const button = dom.appendChild(document.createElement('button'));
		button.type = 'button';
		button.textContent = 'Revert';
		button.onclick = () => revert(view);
		return dom;
	}
}

class Marker extends GutterMarker {
	constructor(kind: Kind) {
		super();
		this.elementClass = `cm-change-${kind}`;
	}
}

const markers: Record<Kind, Marker> = {
	added: new Marker('added'),
	modified: new Marker('modified'),
	deleted: new Marker('deleted')
};

const debounce = ViewPlugin.fromClass(
	class {
		timer = 0;

		constructor(readonly view: EditorView) {}

		update(update: ViewUpdate) {
			if (!update.docChanged || !update.state.field(changes).baseline) return;
			clearTimeout(this.timer);
			this.timer = window.setTimeout(() => this.view.dispatch({ effects: recompute.of(null) }), DEBOUNCE);
		}

		destroy() {
			clearTimeout(this.timer);
		}
	}
);

const style = EditorView.theme({
	'.cm-changeGutter': { width: 'var(--space-3)' },
	'.cm-changeGutter .cm-gutterElement': { position: 'relative' },
	'.cm-change-added, .cm-change-modified, .cm-change-deleted': { cursor: 'pointer' },
	'.cm-change-added': { boxShadow: 'inset 3px 0 var(--git-added)' },
	'.cm-change-modified': { boxShadow: 'inset 3px 0 var(--git-modified)' },
	'.cm-change-deleted::before': {
		content: '""',
		position: 'absolute',
		top: '-4px',
		left: 0,
		borderStyle: 'solid',
		borderWidth: '4px 0 4px 5px',
		borderColor: 'transparent transparent transparent var(--git-deleted)'
	},
	'.cm-changeOriginal': {
		display: 'flex',
		flexDirection: 'column',
		alignItems: 'flex-start',
		gap: 'var(--space-2)',
		padding: 'var(--space-2) 0',
		borderBlock: 'var(--border-width) solid var(--border)',
		background: 'var(--surface-1)'
	},
	'.cm-changeOriginal pre': {
		alignSelf: 'stretch',
		margin: 0,
		padding: '0 2px 0 6px',
		background: 'color-mix(in oklab, var(--git-deleted) 14%, var(--color-bg))',
		font: 'inherit'
	},
	'.cm-changeOriginal button': {
		minHeight: 'var(--target-min)',
		marginLeft: '6px',
		padding: '0 var(--space-3)',
		border: 'var(--border-width) solid var(--border)',
		borderRadius: 'var(--radius-md)',
		background: 'var(--surface-2)',
		color: 'var(--color-fg)',
		font: 'inherit',
		fontFamily: 'var(--font-ui)',
		cursor: 'pointer'
	},
	'.cm-changeOriginal button:hover': { background: 'var(--hover)' },
	'@media (pointer: coarse)': { '.cm-changeGutter': { width: 'var(--space-6)' } }
});

export const changeMarkers = [
	changes,
	debounce,
	style,
	gutter({
		class: 'cm-changeGutter',
		lineMarker(view, line) {
			const chunk = view.state.field(changes).chunks.find((chunk) => covers(chunk, line.from));
			return chunk ? markers[kind(chunk)] : null;
		},
		lineMarkerChange: (update) => update.startState.field(changes) !== update.state.field(changes),
		domEventHandlers: {
			mousedown(view, line) {
				if (!view.state.field(changes).chunks.some((chunk) => covers(chunk, line.from))) return false;
				view.dispatch({ effects: [recompute.of(null), toggle.of(line.from)] });
				return true;
			}
		}
	}),
	Prec.high(
		keymap.of([
			{
				key: 'Escape',
				run(view) {
					if (view.state.field(changes).open === null) return false;
					view.dispatch({ effects: toggle.of(null) });
					return true;
				}
			}
		])
	)
];
