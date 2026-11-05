import type { DraftComment, Hunks, Placement, Side } from '#lib/pr-reviews.ts';
import { getChunks, getOriginalDoc } from '@codemirror/merge';
import { Facet, StateEffect, StateField, type EditorState, type Extension } from '@codemirror/state';
import { BlockType, Decoration, EditorView, GutterMarker, ViewPlugin, WidgetType, gutter, type BlockInfo, type ViewUpdate } from '@codemirror/view';

/** Fills a draft widget's DOM, giving back what tears it down. `fresh` is false when scrolling brought the widget back. */
export type Render = (dom: HTMLElement, shown: { placement: Placement; draft?: DraftComment; lines?: string; fresh: boolean; close(): void }) => () => void;

/**
 * One side of a PR review's Diff tab. A unified view shows the Source, with the Target's deleted lines in widgets.
 * While re-placing, a chosen line goes to `replace` instead of opening a draft.
 */
type Config = { side: Side; unified: boolean; render: Render; replace(placement: Placement): void };

type Drafting = { hunks: Hunks; drafts: DraftComment[]; pending: Placement | null; replacing: boolean };

const config = Facet.define<Config, Config>({ combine: (values) => values[0] });

export const setDrafts = StateEffect.define<DraftComment[]>();
export const openDraft = StateEffect.define<Placement | null>();
export const setReplacing = StateEffect.define<boolean>();

const drafting = StateField.define<Drafting>({
	create: () => ({ hunks: { source: [], target: [] }, drafts: [], pending: null, replacing: false }),
	update(value, tr) {
		for (const effect of tr.effects) {
			if (effect.is(setDrafts)) value = { ...value, drafts: effect.value };
			if (effect.is(openDraft)) value = { ...value, pending: effect.value };
			if (effect.is(setReplacing)) value = { ...value, replacing: effect.value, pending: effect.value ? null : value.pending };
		}
		return value;
	}
});

const setSelecting = StateEffect.define<Placement | null>();

const selecting = StateField.define<Placement | null>({
	create: () => null,
	update(value, tr) {
		for (const effect of tr.effects) if (effect.is(setSelecting)) value = effect.value;
		return value;
	}
});

function active(state: EditorState) {
	return state.field(selecting) ?? state.field(drafting).pending;
}

function hunk(hunks: Hunks, side: Side, line: number) {
	return hunks[side].find(([from, to]) => from <= line && line <= to);
}

function firstLine(placement: Placement) {
	return placement.startLine ?? placement.line;
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
	private shown = false;

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
		const fresh = !this.shown;
		this.shown = true;
		teardowns.set(dom, view.state.facet(config).render(dom, { placement: this.placement, draft: this.draft, lines: this.lines, fresh, close }));
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
			? state.sliceDoc(state.doc.line(firstLine(placement)).from, at.pos)
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

const commentableLine = Decoration.line({ class: 'cm-draftCommentable' });
const savedRange = Decoration.line({ class: 'cm-draftRange' });
const selectedRange = Decoration.line({ class: 'cm-draftSelected' });

function lineMarks(state: EditorState, mark: Decoration, from: number, to: number) {
	return Array.from({ length: Math.max(0, Math.min(to, state.doc.lines) - from + 1) }, (_, index) => mark.range(state.doc.line(from + index).from));
}

/** Marks the lines a re-placed draft can land on. */
const commentableLines = EditorView.decorations.compute([drafting], (state) => {
	const { hunks, replacing } = state.field(drafting);
	if (!replacing) return Decoration.none;
	return Decoration.set(hunks[state.facet(config).side].flatMap(([from, to]) => lineMarks(state, commentableLine, from, to)), true);
});

const rangeLines = EditorView.decorations.compute([drafting, selecting], (state) => {
	const { side } = state.facet(config);
	const current = active(state);
	const ranges = state.field(drafting).drafts.flatMap((draft) => (draft.side === side && draft.startLine ? lineMarks(state, savedRange, draft.startLine, draft.line) : []));
	if (current?.side === side) ranges.push(...lineMarks(state, selectedRange, firstLine(current), current.line));
	return Decoration.set(ranges, true);
});

const replacingClass = EditorView.editorAttributes.compute([drafting], (state) => ({ class: state.field(drafting).replacing ? 'cm-draftReplacing' : '' }));

function spanning(side: Side, anchor: number, head: number, [from, to]: [number, number]): Placement {
	const [start, end] = [Math.max(from, Math.min(anchor, head)), Math.min(to, Math.max(anchor, head))];
	return { side, line: end, startLine: start < end ? start : undefined };
}

function choose(view: EditorView, placement: Placement) {
	if (view.state.field(drafting).replacing) view.state.facet(config).replace(placement);
	else view.dispatch({ effects: openDraft.of(placement) });
}

/** The line a press extends the open draft from. Touch has no Shift, so any tap in the draft's hunk extends it. */
function extending(state: EditorState, side: Side, line: number, event: PointerEvent) {
	const { pending, hunks } = state.field(drafting);
	if (pending?.side !== side) return;
	const start = firstLine(pending);
	const tapped = event.pointerType !== 'mouse' && hunk(hunks, side, start) === hunk(hunks, side, line);
	if (!event.shiftKey && !tapped) return;
	return line < start ? pending.line : start;
}

class Add extends GutterMarker {
	readonly selected: (number | null)[];

	constructor(
		readonly side: Side,
		readonly lines: (number | null)[],
		range: Placement | null
	) {
		super();
		this.selected = range?.side === side ? lines.filter((line) => line !== null && firstLine(range) <= line && line <= range.line) : [];
	}

	eq(other: Add) {
		return other.side === this.side && other.lines.join() === this.lines.join() && other.selected.join() === this.selected.join();
	}

	toDOM() {
		const dom = document.createElement('div');
		dom.className = 'cm-draftAdds';
		for (const line of this.lines) {
			const add = dom.appendChild(document.createElement('div'));
			if (line === null) continue;
			add.className = this.selected.includes(line) ? 'cm-draftAdd cm-draftAddOn' : 'cm-draftAdd';
			add.dataset.side = this.side;
			add.dataset.line = String(line);
			add.title = `Comment on ${this.side === 'target' ? 'deleted ' : ''}line ${line}; drag to select more lines`;
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
		return hunk(view.state.field(drafting).hunks, side, number) ? new Add(side, [number], active(view.state)) : null;
	},
	widgetMarker(view, widget, block) {
		if (!view.state.facet(config).unified || widget instanceof DraftWidget || block.type !== BlockType.WidgetBefore) return null;
		const lines = deletedLines(view.state, block);
		const { hunks } = view.state.field(drafting);
		return lines ? new Add('target', lines.map((line) => (hunk(hunks, 'target', line) ? line : null)), active(view.state)) : null;
	},
	lineMarkerChange: (update) => active(update.startState) !== active(update.state)
});
function pressed(view: EditorView, event: MouseEvent) {
	const target = event.target as Element;
	const add = target.closest<HTMLElement>('.cm-draftAdd');
	if (add) return { side: add.dataset.side as Side, line: Number(add.dataset.line) };
	if (!target.closest('.cm-lineNumbers .cm-gutterElement')) return;
	const block = view.elementAtHeight(event.clientY - view.documentTop);
	if (block.type !== BlockType.Text) return;
	const { side } = view.state.facet(config);
	const line = view.state.doc.lineAt(block.from).number;
	return hunk(view.state.field(drafting).hunks, side, line) ? { side, line } : undefined;
}

/** Nearest rather than under `y`, so a drag can wander off the gutter and over rows it cannot select. */
function nearest(view: EditorView, side: Side, [from, to]: [number, number], y: number) {
	let found: number | undefined;
	let distance = Infinity;
	for (const add of view.dom.querySelectorAll<HTMLElement>(`.cm-draftAdd[data-side="${side}"]`)) {
		const line = Number(add.dataset.line);
		if (line < from || line > to) continue;
		const { top, bottom } = add.getBoundingClientRect();
		const away = Math.max(top - y, y - bottom, 0);
		if (away < distance) [found, distance] = [line, away];
	}
	return found;
}

const SCROLL_EDGE = 32;
const SCROLL_MAX = 40;

/** A mouse drags out a range; a finger only taps, leaving its drags to scroll. */
const pressing = ViewPlugin.fromClass(
	class {
		release?: () => void;

		constructor(readonly view: EditorView) {
			view.dom.addEventListener('pointerdown', this.down);
			view.dom.addEventListener('mousedown', this.hold);
		}

		// Only mousedown can stop a press selecting text or blurring an open draft.
		hold = (event: MouseEvent) => {
			if (pressed(this.view, event)) event.preventDefault();
		};

		down = (event: PointerEvent) => {
			const at = event.button === 0 ? pressed(this.view, event) : undefined;
			if (!at) return;
			const { view } = this;
			const { side } = at;
			const anchor = extending(view.state, side, at.line, event) ?? at.line;
			const range = hunk(view.state.field(drafting).hunks, side, anchor);
			if (!range) return;
			const mouse = event.pointerType === 'mouse';
			const scroller = view.dom.closest('.cm-mergeView') ?? view.scrollDOM;
			const listening = new AbortController();
			let [head, y, frame] = [at.line, event.clientY, 0];

			const follow = () => {
				const line = nearest(view, side, range, y) ?? head;
				if (line === head) return;
				head = line;
				view.dispatch({ effects: setSelecting.of(spanning(side, anchor, head, range)) });
			};
			const tick = () => {
				follow();
				const { top, bottom } = scroller.getBoundingClientRect();
				const over = y < top + SCROLL_EDGE ? y - top - SCROLL_EDGE : y > bottom - SCROLL_EDGE ? y - bottom + SCROLL_EDGE : 0;
				if (over > 0 ? head < range[1] : over < 0 && head > range[0]) scroller.scrollTop += Math.sign(over) * Math.min(SCROLL_MAX, 2 + Math.abs(over) / 2);
				frame = requestAnimationFrame(tick);
			};
			const end = (commit: boolean) => {
				this.release?.();
				if (mouse) view.dispatch({ effects: setSelecting.of(null) });
				if (commit) choose(view, spanning(side, anchor, head, range));
			};

			this.release?.();
			this.release = () => {
				listening.abort();
				cancelAnimationFrame(frame);
				this.release = undefined;
			};
			const { signal } = listening;
			window.addEventListener('pointercancel', () => end(false), { signal });
			window.addEventListener(
				'pointerup',
				(up) => {
					y = up.clientY;
					if (mouse) follow();
					end(true);
				},
				{ signal }
			);
			if (!mouse) return;
			window.addEventListener('pointermove', (move) => (y = move.clientY), { signal });
			view.dispatch({ effects: setSelecting.of(spanning(side, anchor, head, range)) });
			tick();
		};

		destroy() {
			this.release?.();
			this.view.dom.removeEventListener('pointerdown', this.down);
			this.view.dom.removeEventListener('mousedown', this.hold);
		}
	}
);

function tint(percent: number) {
	const color = `color-mix(in oklab, var(--accent) ${percent}%, transparent)`;
	return `linear-gradient(${color}, ${color})`;
}

const fitting = ViewPlugin.fromClass(
	class {
		constructor(readonly view: EditorView) {
			this.fit();
		}

		update(update: ViewUpdate) {
			if (update.geometryChanged) this.fit();
		}

		// Keep widgets within the visible width, since unwrapped long lines widen the content.
		fit() {
			this.view.requestMeasure({
				key: this,
				read: (view) => view.scrollDOM.clientWidth - view.contentDOM.offsetLeft,
				write: (width, view) => view.dom.style.setProperty('--draft-width', `${width}px`)
			});
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
	'.cm-draftAdd:hover, .cm-draftAddOn': { background: 'var(--accent)', color: 'var(--color-bg)', opacity: 1 },
	'.cm-lineNumbers .cm-gutterElement': { cursor: 'pointer' },
	'.cm-draftCommentable': { boxShadow: 'inset 3px 0 var(--accent)' },
	'.cm-draftRange': { backgroundImage: tint(10) },
	'.cm-draftSelected': { backgroundImage: tint(22), boxShadow: 'inset 3px 0 var(--accent)' },
	'&.cm-draftReplacing .cm-draftAdd': { color: 'var(--accent)', opacity: 1 },
	'.cm-draftComment': {
		padding: 'var(--space-2) var(--space-2) var(--space-2) var(--space-3)',
		borderBlock: 'var(--border-width) solid var(--border)',
		background: 'var(--surface-1)',
		whiteSpace: 'normal',
		boxSizing: 'border-box',
		maxWidth: 'var(--draft-width)'
	},
	'@media (pointer: coarse)': { '.cm-draftAdds': { width: 'var(--space-6)' } }
});

/** Draft comments on one side of a PR review's Diff tab, with "+" on the lines GitHub accepts comments on. */
export function draftComments(side: Side, unified: boolean, hunks: Hunks, drafts: DraftComment[], render: Render, replace: Config['replace'], pending: Placement | null): Extension {
	return [
		config.of({ side, unified, render, replace }),
		drafting.init(() => ({ hunks, drafts, pending, replacing: false })),
		selecting,
		widgets,
		commentableLines,
		rangeLines,
		replacingClass,
		adds,
		pressing,
		fitting,
		style
	];
}

export function openPlacement(view: EditorView) {
	return view.state.field(drafting, false)?.pending ?? null;
}

export function openPosition(view: EditorView) {
	const pending = openPlacement(view);
	return pending ? position(view.state, pending)?.pos : undefined;
}

/** The position to scroll to for draft `id` in this view, when it is shown here. */
export function draftPosition(view: EditorView, id: string) {
	const draft = view.state.field(drafting, false)?.drafts.find((draft) => draft.id === id);
	return draft && position(view.state, draft)?.pos;
}
