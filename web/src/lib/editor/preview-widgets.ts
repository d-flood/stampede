import { Facet } from '@codemirror/state';
import { WidgetType, type EditorView } from '@codemirror/view';

export type ImageSource = (src: string) => string | null;

export const imageSource = Facet.define<ImageSource, ImageSource>({
	combine: (values) => values[0] ?? (() => null)
});

function altText(src: string, alt: string) {
	const text = document.createElement('span');
	text.className = 'cm-lp-image-alt';
	text.textContent = alt || src;
	return text;
}

function placeImage(view: EditorView, src: string, alt: string) {
	const url = view.state.facet(imageSource)(src);
	if (!url) return altText(src, alt);
	const image = document.createElement('img');
	image.className = 'cm-lp-image';
	image.alt = alt;
	image.addEventListener('load', () => view.requestMeasure());
	image.addEventListener('error', () => image.replaceWith(altText(src, alt)));
	image.src = url;
	return image;
}

export class ImageWidget extends WidgetType {
	constructor(
		readonly src: string,
		readonly alt: string
	) {
		super();
	}

	eq(other: ImageWidget) {
		return other.src === this.src && other.alt === this.alt;
	}

	ignoreEvent() {
		return false;
	}

	toDOM(view: EditorView) {
		const wrap = document.createElement('span');
		wrap.append(placeImage(view, this.src, this.alt));
		return wrap;
	}
}

export class MarkerWidget extends WidgetType {
	constructor(
		readonly text: string,
		readonly className: string
	) {
		super();
	}

	eq(other: MarkerWidget) {
		return other.text === this.text && other.className === this.className;
	}

	ignoreEvent() {
		return false;
	}

	toDOM() {
		const marker = document.createElement('span');
		marker.className = this.className;
		marker.textContent = this.text;
		return marker;
	}
}

export class TaskWidget extends WidgetType {
	constructor(
		readonly checked: boolean,
		readonly onToggle: (view: EditorView, pos: number) => void
	) {
		super();
	}

	eq(other: TaskWidget) {
		return other.checked === this.checked;
	}

	updateDOM(dom: HTMLElement) {
		dom.querySelector('input')!.checked = this.checked;
		return true;
	}

	toDOM(view: EditorView) {
		const label = document.createElement('label');
		label.className = 'cm-lp-task-box';
		label.addEventListener('mousedown', (event) => event.preventDefault());
		const input = document.createElement('input');
		input.type = 'checkbox';
		input.checked = this.checked;
		input.setAttribute('aria-label', 'Task done');
		input.addEventListener('change', () => this.onToggle(view, view.posAtDOM(label)));
		label.append(input);
		return label;
	}
}

export class ChipWidget extends WidgetType {
	constructor(
		readonly open: boolean,
		readonly onToggle: (view: EditorView) => void
	) {
		super();
	}

	eq(other: ChipWidget) {
		return other.open === this.open;
	}

	toDOM(view: EditorView) {
		const row = document.createElement('div');
		row.className = 'cm-lp-chip-row';
		const chip = document.createElement('button');
		chip.type = 'button';
		chip.className = 'cm-lp-chip';
		chip.textContent = 'Front matter';
		chip.setAttribute('aria-expanded', String(this.open));
		chip.addEventListener('mousedown', (event) => event.preventDefault());
		chip.addEventListener('click', () => this.onToggle(view));
		row.append(chip);
		return row;
	}
}

export type InlineNode =
	| { kind: 'text'; text: string }
	| { kind: 'em' | 'strong' | 's' | 'code' | 'link'; children: InlineNode[] }
	| { kind: 'sup'; text: string }
	| { kind: 'image'; src: string; alt: string };

export type TableCell = { offset: number; content: InlineNode[] };
export type TableModel = {
	aligns: (string | null)[];
	head: (TableCell | null)[];
	rows: (TableCell | null)[][];
};

const INLINE_TAGS = { em: 'em', strong: 'strong', s: 's', code: 'code', link: 'span' } as const;

function appendInline(view: EditorView, parent: HTMLElement, nodes: InlineNode[]) {
	for (const node of nodes) {
		if (node.kind === 'text') {
			parent.append(node.text);
		} else if (node.kind === 'image') {
			parent.append(placeImage(view, node.src, node.alt));
		} else if (node.kind === 'sup') {
			const sup = document.createElement('sup');
			sup.className = 'cm-lp-fnref';
			sup.textContent = node.text;
			parent.append(sup);
		} else {
			const element = document.createElement(INLINE_TAGS[node.kind]);
			if (node.kind === 'link') element.className = 'cm-lp-link';
			if (node.kind === 'code') element.className = 'cm-lp-inline-code';
			appendInline(view, element, node.children);
			parent.append(element);
		}
	}
}

export class TableWidget extends WidgetType {
	constructor(
		readonly source: string,
		readonly model: TableModel
	) {
		super();
	}

	eq(other: TableWidget) {
		return other.source === this.source;
	}

	get estimatedHeight() {
		return (this.model.rows.length + 1) * 34;
	}

	toDOM(view: EditorView) {
		const wrap = document.createElement('div');
		wrap.className = 'cm-lp-table-wrap';
		const table = document.createElement('table');
		table.className = 'cm-lp-table';

		const row = (cells: (TableCell | null)[], tag: 'th' | 'td') => {
			const tr = document.createElement('tr');
			cells.forEach((cell, index) => {
				const element = document.createElement(tag);
				const align = this.model.aligns[index];
				if (align) element.style.textAlign = align;
				if (cell) {
					element.dataset.offset = String(cell.offset);
					appendInline(view, element, cell.content);
				}
				tr.append(element);
			});
			return tr;
		};

		const head = document.createElement('thead');
		head.append(row(this.model.head, 'th'));
		const body = document.createElement('tbody');
		for (const cells of this.model.rows) body.append(row(cells, 'td'));
		table.append(head, body);
		wrap.append(table);

		wrap.addEventListener('click', (event) => {
			const cell = (event.target as HTMLElement).closest<HTMLElement>('[data-offset]');
			const anchor = view.posAtDOM(wrap) + Number(cell?.dataset.offset ?? 0);
			view.dispatch({ selection: { anchor } });
			view.focus();
		});
		return wrap;
	}
}
