import type { DraftComment, Hunks, Placement, Side } from '#lib/pr-reviews.ts';
import { getChunks, getOriginalDoc } from '@codemirror/merge';
import { Facet, StateEffect, StateField, type EditorState, type Extension } from '@codemirror/state';
import { BlockType, Decoration, EditorView, GutterMarker, ViewPlugin, WidgetType, gutter, type BlockInfo } from '@codemirror/view';

/** Fills a draft widget's DOM, giving back what tears it down. */
export type Render = (dom: HTMLElement, shown: { placement: Placement; draft?: DraftComment; lines?: string; close(): void }) => () => void;

/** One side of a PR review's Diff tab. A unified view shows the Source, with the Target's deleted lines in widgets. */
type Config = { side: Side; unified: boolean; render: Render };

type Drafting = { hunks: Hunks; drafts: DraftComment[]; pending: Placement | null };

const config = Facet.define<Config, Config>({ combine: (values) => values[0] });

export const setDrafts = StateEffect.define<DraftComment[]>();
export const openDraft = StateEffect.define<Placement | null>();

const drafting = StateField.define<Drafting>({
	create: () => ({ hunks: { source: [], target: [] }, drafts: [], pending: null }),
	update(value, tr) {
		for (const effect of tr.effects) {
			if (effect.is(setDrafts)) value = { ...value, drafts: effect.value };
			if (effect.is(openDraft)) value = { ...value, pending: effect.value };
		}
		return value;
	}
});

function hunk(hunks: Hunks, side: Side, line: number) {
	return hunks[side].find(([from, to]) => from <= line && line <= to);
}

/** Where a placement's widget goes in this view: after its last line, or after the deleted chunk holding it. */
function position(state: EditorState, { side, line }: Placement): { pos: number; side: number } | undefined {
	const own = state.facet(config);
	if (side === own.side) return line <= state.doc.lines ? { pos: state.doc.line(line).to, side: 1 } : undefined;
	if (!own.unified) return;
	const original = getOriginalDoc(state);
	if (line > original.lines) return;
	const at = original.line(line).from;
	const chunks = getChunks(state)?.chunks ?? [];
	const deleted = chunks.find((chunk) => chunk.fromA <= at && at < chunk.toA);
	// Side 0 sorts after the deletion widget's side -1, so the draft sits under its deleted lines.
	if (deleted) return { pos: deleted.fromB, side: 0 };
	const before = chunks.findLast((chunk) => chunk.toA <= at);
	const mapped = Math.min(state.doc.length, before ? at - before.toA + before.toB : at);
	return { pos: state.doc.lineAt(mapped).to, side: 1 };
}

const teardowns = new WeakMap<HTMLElement, () => void>();

class DraftWidget extends WidgetType {
	constructor(
		readonly key: string,
		readonly placement: Placement,
		readonly draft: DraftComment | undefined,
		readonly lines: string | undefined
	) {
		super();
	}

	eq(other: DraftWidget) {
		return other.key === this.key;
	}

	toDOM(view: EditorView) {
		const dom = document.createElement('div');
		dom.className = 'cm-draftComment';
		if (this.draft) dom.dataset.draft = this.draft.id;
		const close = () => view.dispatch({ effects: openDraft.of(null) });
		teardowns.set(dom, view.state.facet(config).render(dom, { placement: this.placement, draft: this.draft, lines: this.lines, close }));
		return dom;
	}

	destroy(dom: HTMLElement) {
		teardowns.get(dom)?.();
	}

	get estimatedHeight() {
		return 120;
	}

	ignoreEvent() {
		return true;
	}
}

function widget(state: EditorState, placement: Placement, draft?: DraftComment) {
	const at = position(state, placement);
	if (!at) return;
	const lines =
		placement.side === 'source' && state.facet(config).side === 'source'
			? state.sliceDoc(state.doc.line(placement.startLine ?? placement.line).from, at.pos)
			: undefined;
	const key = JSON.stringify([draft?.id, placement.side, placement.startLine, placement.line, draft?.body]);
	return Decoration.widget({ widget: new DraftWidget(key, placement, draft, lines), block: true, side: at.side }).range(at.pos);
}

const widgets = EditorView.decorations.compute([drafting], (state) => {
	const { drafts, pending } = state.field(drafting);
	const ranges = drafts.map((draft) => widget(state, draft, draft));
	if (pending) ranges.push(widget(state, pending));
	return Decoration.set(ranges.filter((range) => !!range), true);
});

/** Opens a draft on `line`, or on the range from `anchor` to it, kept within `line`'s hunk. */
function choose(view: EditorView, side: Side, line: number, anchor = line) {
	const range = hunk(view.state.field(drafting).hunks, side, line);
	if (!range) return;
	const start = Math.max(range[0], Math.min(anchor, line));
	const end = Math.min(range[1], Math.max(anchor, line));
	view.dispatch({ effects: openDraft.of({ side, line: end, startLine: start < end ? start : undefined }) });
}

/** The line a shift-click extends from: the open draft's first line when it is on that side. */
function anchor(view: EditorView, side: Side, event: MouseEvent) {
	const { pending } = view.state.field(drafting);
	return event.shiftKey && pending?.side === side ? (pending.startLine ?? pending.line) : undefined;
}

class Add extends GutterMarker {
	constructor(
		readonly side: Side,
		readonly lines: (number | null)[]
	) {
		super();
	}

	eq(other: Add) {
		return other.side === this.side && other.lines.join() === this.lines.join();
	}

	toDOM() {
		const dom = document.createElement('div');
		dom.className = 'cm-draftAdds';
		for (const line of this.lines) {
			const add = dom.appendChild(document.createElement('div'));
			if (line === null) continue;
			add.className = 'cm-draftAdd';
			add.dataset.side = this.side;
			add.dataset.line = String(line);
			add.title = `Comment on ${this.side === 'target' ? 'deleted ' : ''}line ${line}`;
			add.textContent = '+';
		}
		return dom;
	}
}

/** The deleted Target lines a unified view's deletion widget at `block` shows. */
function deletedLines(state: EditorState, block: BlockInfo) {
	const chunk = getChunks(state)?.chunks.find((chunk) => chunk.fromB === block.from && chunk.fromA < chunk.toA);
	if (!chunk) return;
	const original = getOriginalDoc(state);
	const [first, last] = [original.lineAt(chunk.fromA).number, original.lineAt(Math.max(chunk.fromA, chunk.toA - 1)).number];
	return Array.from({ length: last - first + 1 }, (_, index) => first + index);
}

const adds = gutter({
	class: 'cm-draftGutter',
	lineMarker(view, line) {
		const { side } = view.state.facet(config);
		const number = view.state.doc.lineAt(line.from).number;
		return hunk(view.state.field(drafting).hunks, side, number) ? new Add(side, [number]) : null;
	},
	widgetMarker(view, widget, block) {
		if (!view.state.facet(config).unified || widget instanceof DraftWidget || block.type !== BlockType.WidgetBefore) return null;
		const lines = deletedLines(view.state, block);
		const { hunks } = view.state.field(drafting);
		return lines ? new Add('target', lines.map((line) => (hunk(hunks, 'target', line) ? line : null))) : null;
	},
	domEventHandlers: {
		mousedown(view, _block, event) {
			const add = (event.target as Element).closest<HTMLElement>('.cm-draftAdd');
			if (!add || !(event instanceof MouseEvent) || event.button !== 0) return false;
			const side = add.dataset.side as Side;
			const line = Number(add.dataset.line);
			const from = anchor(view, side, event) ?? line;
			event.preventDefault();
			window.addEventListener(
				'mouseup',
				(up) => {
					const over = document.elementFromPoint(up.clientX, up.clientY)?.closest<HTMLElement>('.cm-draftAdd');
					const dragged = over && view.dom.contains(over) && over.dataset.side === side ? Number(over.dataset.line) : line;
					choose(view, side, dragged, dragged === line ? from : line);
				},
				{ once: true }
			);
			return true;
		}
	}
});

/** Tapping a commentable line's number opens a draft there, which is how a phone comments. */
const lineNumbers = ViewPlugin.fromClass(
	class {
		constructor(readonly view: EditorView) {
			view.dom.addEventListener('click', this.click);
		}

		click = (event: MouseEvent) => {
			if (!(event.target as Element).closest('.cm-lineNumbers .cm-gutterElement')) return;
			const block = this.view.elementAtHeight(event.clientY - this.view.documentTop);
			if (block.type !== BlockType.Text) return;
			const { side } = this.view.state.facet(config);
			const line = this.view.state.doc.lineAt(block.from).number;
			choose(this.view, side, line, anchor(this.view, side, event));
		};

		destroy() {
			this.view.dom.removeEventListener('click', this.click);
		}
	}
);

const style = EditorView.theme({
	'.cm-draftGutter .cm-gutterElement': { display: 'flex', flexDirection: 'column' },
	'.cm-draftAdds': { display: 'flex', flex: 1, flexDirection: 'column', width: '1.25em' },
	'.cm-draftAdds > div': { flex: 1 },
	'.cm-draftAdd': {
		display: 'flex',
		alignItems: 'center',
		justifyContent: 'center',
		borderRadius: 'var(--radius-sm)',
		color: 'var(--muted-fg)',
		cursor: 'pointer',
		opacity: 0.45,
		userSelect: 'none'
	},
	'.cm-draftAdd:hover': { background: 'var(--status-working)', color: 'var(--color-bg)', opacity: 1 },
	'.cm-lineNumbers .cm-gutterElement': { cursor: 'pointer' },
	'.cm-draftComment': {
		padding: 'var(--space-2) var(--space-2) var(--space-2) var(--space-3)',
		borderBlock: 'var(--border-width) solid var(--border)',
		background: 'var(--surface-1)',
		whiteSpace: 'normal'
	},
	'@media (pointer: coarse)': { '.cm-draftAdds': { width: 'var(--space-6)' } }
});

/** Draft comments on one side of a PR review's Diff tab, with "+" on the lines GitHub accepts comments on. */
export function draftComments(side: Side, unified: boolean, hunks: Hunks, drafts: DraftComment[], render: Render): Extension {
	return [
		config.of({ side, unified, render }),
		drafting.init(() => ({ hunks, drafts, pending: null })),
		widgets,
		adds,
		lineNumbers,
		style
	];
}

/** The position to scroll to for draft `id` in this view, when it is shown here. */
export function draftPosition(view: EditorView, id: string) {
	const draft = view.state.field(drafting, false)?.drafts.find((draft) => draft.id === id);
	return draft && position(view.state, draft)?.pos;
}
