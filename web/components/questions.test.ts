import RequestCard from '#lib/chat/RequestCard.svelte';
import Questions from '#lib/chat/Questions.svelte';
import { clearDraft, loadDraft, redirect } from '#lib/chat/questions.ts';
import type { Question, QuestionResponse, Request } from '#lib/device.svelte.ts';
import { afterEach, expect, test, vi } from 'vitest';
import { userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

const NOTE = 'Add a note or your own answer';
const questions: Question[] = [
	{ id: 'storage', header: 'Storage', question: '**Choose storage**\n\n```ts\nconst local = true;\n```', type: 'single', options: [{ id: 'local', label: 'Local', description: 'Keep data offline.', preview: 'local = true', recommended: true }, { id: 'remote', label: 'Remote', description: 'Use hosted storage.' }], multiSelect: false },
	{ id: 'tools', header: 'Tools', question: 'Choose tools.', type: 'multiple', options: [{ id: 'read', label: 'Read', description: '' }, { id: 'write', label: 'Write', description: '' }], multiSelect: true },
	{ id: 'notes', header: 'Notes', question: 'Describe it.', type: 'text', options: [], multiSelect: false },
	{ id: 'optional', header: 'Optional', question: 'Anything else?', type: 'text', options: [], multiSelect: false }
];
const single: Question = { ...questions[0], header: 'Migration', question: 'How should it run?' };
const legacy: Question[] = [
	{ id: '', header: 'Color', question: 'Choose a color', options: [{ label: 'Red', description: 'Warm' }, { label: 'Blue', description: 'Cool' }], multiSelect: false },
	{ id: '', header: 'Shape', question: 'Choose shapes', options: [{ label: 'Circle', description: 'Round' }, { label: 'Square', description: 'Four sides' }], multiSelect: true }
];

const request = (list: Question[], id = 'questions'): Request => ({ kind: 'question', id, questions: list });
async function open(list: Question[], { id = 'questions', disabled = false } = {}) {
	const onanswer = vi.fn();
	const view = await render(RequestCard, { request: request(list, id), agent: 'Claude', disabled, onanswer });
	return { view, onanswer, row: (title: string) => view.getByRole('button', { name: new RegExp(`^${title}`) }), send: view.getByRole('button', { name: /^Send/ }) };
}

afterEach(() => localStorage.clear());

test('answers collapse into rows, stay editable, and send with anything unanswered skipped', async () => {
	const { view, onanswer, row, send } = await open(questions);
	expect(view.container.querySelector('strong')?.textContent).toBe('Choose storage');
	expect(view.container.querySelector('pre code')?.textContent).toBe('const local = true;\n');
	expect(view.container.querySelector('.preview')?.textContent).toBe('local = true');
	await expect.element(send).toBeDisabled();
	await expect.element(view.getByRole('radio', { name: 'Local, recommended', exact: true })).toBeVisible();

	await view.getByText('Keep data offline.').click();
	await expect.element(row('Storage')).toMatchTextContent('Local');
	await expect.element(row('Tools')).toHaveAttribute('aria-expanded', 'true');
	await view.getByRole('checkbox', { name: 'Read', exact: true }).click();
	await view.getByRole('checkbox', { name: 'Write', exact: true }).click();
	await view.getByRole('textbox', { name: NOTE }).fill('Ask before deleting.');
	await expect.element(row('Tools')).toHaveAttribute('aria-expanded', 'true');
	await view.getByRole('button', { name: 'Next question', exact: true }).click();
	await view.getByRole('textbox', { name: 'Your answer', exact: true }).fill('First line\nSecond line');
	await expect.element(send).toMatchTextContent('Send 3, skip 1');
	await expect.element(row('Optional')).toMatchTextContent('Not answered');

	await row('Storage').click();
	await view.getByRole('textbox', { name: NOTE }).fill('Keep offline support.');
	await view.getByRole('radio', { name: 'Remote', exact: true }).click();
	await expect.element(row('Storage')).toMatchTextContent('Remote. Keep offline support.');
	await expect.element(row('Optional')).toHaveAttribute('aria-expanded', 'true');
	await expect.poll(() => view.container.scrollWidth <= view.container.clientWidth).toBe(true);
	await send.click();
	expect(onanswer).toHaveBeenCalledExactlyOnceWith({ responses: {
		storage: { selected: ['remote'], text: 'Keep offline support.', skipped: false },
		tools: { selected: ['read', 'write'], text: 'Ask before deleting.', skipped: false },
		notes: { selected: [], text: 'First line\nSecond line', skipped: false },
		optional: { selected: [], text: '', skipped: true }
	} });
});

test('a row closes on a second click, and clearing an answer reopens the choice', async () => {
	const { view, row, send } = await open(questions);
	await expect.element(row('Storage')).toHaveAttribute('aria-expanded', 'true');
	await row('Storage').click();
	await expect.element(row('Storage')).toHaveAttribute('aria-expanded', 'false');
	await expect.element(view.getByRole('radio')).not.toBeInTheDocument();
	await row('Storage').click();
	await view.getByRole('radio', { name: 'Remote', exact: true }).click();
	await expect.element(row('Storage')).toMatchTextContent('Remote');
	await row('Storage').click();
	await view.getByRole('button', { name: 'Dismiss', exact: true }).click();
	await expect.element(view.getByRole('button', { name: 'Discard 1 answer?', exact: true })).toBeVisible();
	await view.getByRole('button', { name: 'Clear answer', exact: true }).click();
	await expect.element(view.getByRole('button', { name: 'Dismiss', exact: true })).toBeVisible();
	await expect.element(view.getByRole('radio', { name: 'Remote', exact: true })).not.toBeChecked();
	await expect.element(send).toBeDisabled();
});

test('the last answer collapses every row and hands focus to Send', async () => {
	const { view, send } = await open(questions.slice(0, 2));
	await view.getByRole('radio', { name: 'Remote', exact: true }).click();
	await view.getByRole('checkbox', { name: 'Read', exact: true }).click();
	await expect.element(view.getByRole('button', { name: 'Next question' })).not.toBeInTheDocument();
	await userEvent.keyboard('{Enter}');
	await expect.element(view.getByRole('checkbox')).not.toBeInTheDocument();
	if (!matchMedia('(pointer: coarse) and (max-width: 767px)').matches) await expect.element(send).toHaveFocus();
});

test('a single question has no rows and sends in one step', async () => {
	const { view, onanswer, send } = await open([single]);
	await expect.element(view.getByRole('heading', { name: 'Migration' })).toBeVisible();
	await expect.element(send).toMatchTextContent('Send answer');
	await expect.element(send).toBeDisabled();
	expect(view.container.querySelector('.row')).toBeNull();
	await view.getByRole('radio', { name: 'Remote', exact: true }).click();
	await expect.element(view.getByRole('radio', { name: 'Remote', exact: true })).toBeChecked();
	await userEvent.keyboard('{Enter}');
	expect(onanswer).toHaveBeenCalledExactlyOnceWith({ responses: { storage: { selected: ['remote'], text: '', skipped: false } } });
});

test('number keys work once the form has focus', async () => {
	const { view, onanswer } = await open([single]);
	view.component.focus();
	await userEvent.keyboard('2');
	await expect.element(view.getByRole('radio', { name: 'Remote', exact: true })).toBeChecked();
	await view.getByRole('textbox', { name: NOTE }).fill('1');
	await expect.element(view.getByRole('radio', { name: 'Remote', exact: true })).toBeChecked();
	expect(onanswer).not.toHaveBeenCalled();
});

test('options with images become side-by-side tiles', async () => {
	const image = { title: 'Layout', asset: 'missing.png' };
	const { view } = await open([{ ...single, options: [{ id: 'a', label: 'Footnotes', description: '', image }, { id: 'b', label: 'Columns', description: '', image }] }]);
	await expect.element(view.getByRole('radio', { name: 'Columns', exact: true })).toBeVisible();
	const [first, second] = view.container.querySelectorAll('.option');
	await expect.poll(() => first.getBoundingClientRect().top === second.getBoundingClientRect().top && first.getBoundingClientRect().left < second.getBoundingClientRect().left).toBe(true);
	await view.getByText('Columns').click();
	await expect.element(view.getByRole('radio', { name: 'Columns', exact: true })).toBeChecked();
	await expect.element(view.getByRole('radio', { name: 'Footnotes', exact: true })).not.toBeChecked();
});

test('questions without rich fields use the same form and answer by label', async () => {
	const { view, onanswer, row, send } = await open(legacy);
	await view.getByRole('radio', { name: 'Red', exact: true }).click();
	await expect.element(row('Color')).toMatchTextContent('Red');
	await view.getByRole('checkbox', { name: 'Circle', exact: true }).click();
	await view.getByRole('checkbox', { name: 'Square', exact: true }).click();
	await view.getByRole('textbox', { name: NOTE }).fill('Triangle');
	await send.click();
	expect(onanswer).toHaveBeenCalledExactlyOnceWith({ answers: [['Red'], ['Circle', 'Square', 'Triangle']] });
});

test('a question with no header or options still reads and answers', async () => {
	const { view, onanswer, send } = await open([{ id: '', header: '', question: 'What now?', options: [], multiSelect: false }]);
	await expect.element(view.getByRole('heading', { name: 'Question' })).toBeVisible();
	await view.getByRole('textbox', { name: 'Your answer', exact: true }).fill('Ship it');
	await send.click();
	expect(onanswer).toHaveBeenCalledExactlyOnceWith({ answers: [['Ship it']] });
});

test('dismissing is immediate with no answers and asks first once there are some', async () => {
	const first = await open(questions, { id: 'empty' });
	await first.view.getByRole('button', { name: 'Dismiss', exact: true }).click();
	expect(first.onanswer).toHaveBeenCalledExactlyOnceWith({ allow: false });
	await first.view.unmount();

	const { view, onanswer, row } = await open(questions, { id: 'answered' });
	await view.getByRole('radio', { name: 'Remote', exact: true }).click();
	await expect.element(row('Storage')).toMatchTextContent('Remote');
	await view.getByRole('button', { name: 'Dismiss', exact: true }).click();
	expect(onanswer).not.toHaveBeenCalled();
	await view.getByRole('button', { name: 'Discard 1 answer?', exact: true }).click();
	expect(onanswer).toHaveBeenCalledExactlyOnceWith({ allow: false });
});

test('disabled forms keep their answers but accept no input', async () => {
	const { view, onanswer, send } = await open(questions, { disabled: true });
	for (const input of view.getByRole('radio').elements()) expect(input).toBeDisabled();
	await expect.element(view.getByRole('textbox', { name: NOTE })).toBeDisabled();
	await expect.element(view.getByRole('button', { name: 'Dismiss', exact: true })).toBeDisabled();
	await expect.element(send).toBeDisabled();
	expect(onanswer).not.toHaveBeenCalled();
});

test('drafts restore after a remount and travel with a redirect', async () => {
	const first = await open(questions, { id: 'draft' });
	await first.view.getByRole('radio', { name: 'Remote', exact: true }).click();
	await expect.element(first.row('Storage')).toMatchTextContent('Remote');
	await first.view.unmount();

	const { row } = await open(questions, { id: 'draft' });
	await expect.element(row('Storage')).toMatchTextContent('Remote');
	await expect.element(row('Tools')).toHaveAttribute('aria-expanded', 'true');
	expect(redirect(request(questions, 'draft'), 'Stop.')).toEqual({ allow: false, instead: 'Stop.', responses: { storage: { selected: ['remote'], text: '', skipped: false } } });
	expect(redirect(request(legacy, 'draft'), 'Stop.')).toEqual({ allow: false, instead: 'Stop.' });
	clearDraft('draft');
	expect(loadDraft('draft')).toEqual({});
});

const responses: Record<string, QuestionResponse> = {
	storage: { selected: ['remote'], text: 'Keep offline support.', skipped: false },
	tools: { selected: ['read', 'write'], text: '', skipped: false },
	notes: { selected: [], text: 'First line\nSecond line', skipped: false },
	optional: { selected: [], text: '', skipped: true }
};

test('history shows answers while collapsed and every option when open', async () => {
	const view = await render(Questions, { record: { after: 1, questions, answers: [], responses } });
	const group = view.getByRole('group', { name: 'Answered questions', exact: true });
	const summary = view.container.querySelector('summary')!;
	expect(summary.textContent).toContain('3 of 4 answered');
	expect(summary.textContent).toContain('Storage: Remote. Keep offline support.');
	expect(summary.textContent).toContain('Tools: Read, Write');
	expect(summary.textContent).toContain('Optional: skipped');
	summary.click();
	await expect.element(group.getByText('Keep offline support.', { exact: true })).toBeVisible();
	await expect.element(group.getByText('Use hosted storage.', { exact: true })).toBeVisible();
	await expect.element(group.getByText('Skipped', { exact: true })).toBeVisible();
	expect(Array.from(view.container.querySelectorAll('.option.selected strong')).map((node) => node.textContent)).toEqual(['Remote', 'Read', 'Write']);
});

test('history records dismissals and instructions', async () => {
	const dismissed = await render(Questions, { record: { after: 1, questions, answers: [], cancelled: true } });
	await expect.element(dismissed.getByRole('group', { name: 'Unanswered questions', exact: true })).toBeVisible();
	expect(dismissed.container.querySelector('summary')!.textContent).toContain('4 questions dismissed');
	expect(dismissed.container.querySelector('.instruction')).toBeNull();
	await dismissed.unmount();

	const redirected = await render(Questions, { record: { after: 1, questions, answers: [], responses: { storage: responses.storage }, cancelled: true, instruction: 'Use the defaults.' } });
	expect(redirected.container.querySelector('summary')!.textContent).toContain('1 of 4 answered');
	await expect.element(redirected.getByText('Use the defaults.', { exact: true })).toBeVisible();
	await redirected.unmount();

	const unanswered = await render(Questions, { record: { after: 1, questions: [single], answers: [], cancelled: true, instruction: 'Never mind.' } });
	expect(unanswered.container.querySelector('summary')!.textContent).toContain('1 question not answered');
});

test('history reads older records that stored answers by label', async () => {
	const view = await render(Questions, { record: { after: 1, questions: legacy, answers: [['Red'], ['Circle', 'Triangle']] } });
	const summary = view.container.querySelector('summary')!;
	expect(summary.textContent).toContain('2 questions answered');
	expect(summary.textContent).toContain('Color: Red');
	expect(summary.textContent).toContain('Shape: Circle. Triangle');
	summary.click();
	await expect.element(view.getByText('Triangle', { exact: true })).toBeVisible();
	expect(Array.from(view.container.querySelectorAll('.option.selected strong')).map((node) => node.textContent)).toEqual(['Red', 'Circle']);
});
