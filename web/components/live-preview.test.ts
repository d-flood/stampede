import { language } from '#lib/editor/languages.ts';
import { livePreview } from '#lib/editor/live-preview.ts';
import { theme } from '#lib/editor/theme.ts';
import { EditorState } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { afterEach, expect, test } from 'vitest';
import { page } from 'vitest/browser';

const doc = ['# Title', '', 'Some **bold** words.', '', '- item one', '- [ ] task', '', '| A | B |', '| - | - |', '| 1 | 2 |', '', 'End.'].join('\n');
let view: EditorView;

async function mount(text = doc, options?: Parameters<typeof livePreview>[0]) {
	const parent = document.body.appendChild(document.createElement('div'));
	view = new EditorView({ parent, state: EditorState.create({ doc: text, extensions: [await language('notes.md'), livePreview(options), theme] }) });
}

function line(number: number) {
	let node: Node | null = view.domAtPos(view.state.doc.line(number).from).node;
	while (node && !(node instanceof HTMLElement && node.classList.contains('cm-line'))) node = node.parentNode;
	return node as HTMLElement;
}

async function touch(number: number) {
	view.focus();
	view.dispatch({ selection: { anchor: view.state.doc.line(number).from } });
	await expect.poll(() => view.hasFocus).toBe(true);
}

afterEach(() => {
	view.dom.parentElement?.remove();
	view.destroy();
});

test('formats every line of an unfocused editor', async () => {
	await mount();
	await expect.poll(() => line(1).textContent).toBe('Title');
	expect(line(1).classList).toContain('cm-lp-h1');
	expect(parseFloat(getComputedStyle(line(1)).fontSize)).toBeGreaterThan(parseFloat(getComputedStyle(line(3)).fontSize));
	expect(line(3).textContent).toBe('Some bold words.');
	const bold = [...line(3).querySelectorAll('span')].find((span) => span.textContent === 'bold')!;
	expect(Number(getComputedStyle(bold).fontWeight)).toBeGreaterThanOrEqual(700);
	expect(line(5).textContent).toBe('•item one');
	expect(line(5).querySelector('.cm-lp-bullet')).not.toBeNull();
	await expect.element(page.getByRole('checkbox', { name: 'Task done' })).not.toBeChecked();
	expect(line(6).textContent).toBe('task');
	await expect.element(page.getByRole('cell', { name: '2', exact: true })).toBeVisible();
	expect(view.contentDOM.textContent).not.toContain('|');
	expect(getComputedStyle(line(3)).fontFamily).toContain('Literata');
});

test('shows the source of the touched lines only', async () => {
	await mount();
	await touch(1);
	await expect.poll(() => line(1).textContent).toBe('# Title');
	expect(line(3).textContent).toBe('Some bold words.');
	await touch(3);
	await expect.poll(() => line(3).textContent).toBe('Some **bold** words.');
	expect(line(1).textContent).toBe('Title');
	await touch(5);
	await expect.poll(() => line(5).textContent).toBe('- item one');
	await touch(9);
	await expect.poll(() => line(8).textContent).toBe('| A | B |');
	await expect.element(page.getByRole('table')).not.toBeInTheDocument();
	view.contentDOM.blur();
	await expect.poll(() => line(3).textContent).toBe('Some bold words.');
	await expect.element(page.getByRole('table')).toBeVisible();
});

test('clicking a task checkbox toggles it in the document', async () => {
	await mount();
	const checkbox = page.getByRole('checkbox', { name: 'Task done' });
	await checkbox.click();
	await expect.poll(() => view.state.doc.line(6).text).toBe('- [x] task');
	await expect.element(checkbox).toBeChecked();
	await checkbox.click();
	await expect.poll(() => view.state.doc.line(6).text).toBe('- [ ] task');
	await expect.element(checkbox).not.toBeChecked();
});

test('shows full, collapsed and shortcut reference images from their definitions', async () => {
	const gif = 'data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7';
	const text = ['![Full][Pic]', '', '![pic][]', '', '![pic]', '', '![Missing][nope]', '', `[pic]: <${gif}>`].join('\n');
	await mount(text, { image: (src) => src });
	const images = () => [...view.contentDOM.querySelectorAll<HTMLImageElement>('img.cm-lp-image')].map((image) => [image.alt, image.getAttribute('src'), image.complete && image.naturalWidth]);
	await expect.poll(images).toEqual([['Full', gif, 1], ['pic', gif, 1], ['pic', gif, 1]]);
	expect(line(7).textContent).toBe('![Missing][nope]');
});
