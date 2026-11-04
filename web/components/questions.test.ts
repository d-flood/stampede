import RequestCard from '#lib/chat/RequestCard.svelte';
import Questions from '#lib/chat/Questions.svelte';
import type { Question, QuestionResponse } from '#lib/device.svelte.ts';
import { expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

const questions: Question[] = [
	{ id: 'storage', header: 'Storage', question: '**Choose storage**\n\n```ts\nconst local = true;\n```', type: 'single', options: [{ id: 'local', label: 'Local', description: 'Keep data offline.', preview: 'local = true', recommended: true }, { id: 'remote', label: 'Remote', description: 'Use hosted storage.' }], multiSelect: false },
	{ id: 'tools', header: 'Tools', question: 'Choose tools.', type: 'multiple', options: [{ id: 'read', label: 'Read', description: '' }, { id: 'write', label: 'Write', description: '' }], multiSelect: true },
	...['notes', 'alternative', 'optional', 'finish'].map((id): Question => ({ id, header: id, question: id, type: id === 'alternative' ? 'single' : 'text', options: [], multiSelect: false }))
];

test('rich questions retain edits, validate completion, review, and render answer history', async () => {
	const onanswer = vi.fn();
	const view = await render(RequestCard, { request: { kind: 'question', id: 'questions', questions }, agent: 'Claude', disabled: false, onanswer });
	expect(view.container.querySelector('strong')?.textContent).toBe('Choose storage');
	expect(view.container.querySelector('pre code')?.textContent).toBe('const local = true;\n');
	expect(view.container.querySelector('.preview')?.textContent).toBe('local = true');
	expect(view.getByRole('navigation', { name: 'Questions' }).elements()).toHaveLength(matchMedia('(pointer: coarse) and (max-width: 767px)').matches ? 0 : 1);
	await expect.element(view.getByRole('button', { name: 'Next', exact: true })).toBeDisabled();
	await view.getByRole('button', { name: 'Local Recommended', exact: true }).click();
	await view.getByRole('textbox', { name: 'Additional text (optional)' }).fill('Keep offline support.');
	await view.getByRole('button', { name: 'Next', exact: true }).click();
	await view.getByRole('button', { name: 'Read', exact: true }).click();
	await view.getByRole('button', { name: 'Write', exact: true }).click();
	await view.getByRole('textbox', { name: 'Additional text (optional)' }).fill('Ask before deleting.');
	await view.getByRole('button', { name: 'Back', exact: true }).click();
	await expect.element(view.getByRole('textbox', { name: 'Additional text (optional)' })).toHaveValue('Keep offline support.');
	await expect.element(view.getByRole('button', { name: 'Local Recommended', exact: true })).toHaveAttribute('aria-pressed', 'true');
	await view.getByRole('button', { name: 'Next', exact: true }).click();
	await expect.element(view.getByRole('button', { name: 'Read', exact: true })).toHaveAttribute('aria-pressed', 'true');
	await expect.element(view.getByRole('button', { name: 'Write', exact: true })).toHaveAttribute('aria-pressed', 'true');
	await expect.element(view.getByRole('textbox', { name: 'Additional text (optional)' })).toHaveValue('Ask before deleting.');
	await view.getByRole('button', { name: 'Next', exact: true }).click();
	await view.getByRole('textbox', { name: 'Answer', exact: true }).fill('First line\nSecond line');
	await view.getByRole('button', { name: 'Next', exact: true }).click();
	await view.getByRole('textbox', { name: 'Additional text (optional)' }).fill('A custom alternative');
	await view.getByRole('button', { name: 'Next', exact: true }).click();
	await view.getByRole('button', { name: 'Skip', exact: true }).click();
	await view.getByRole('textbox', { name: 'Answer', exact: true }).fill('Done');
	await view.getByRole('button', { name: 'Review answers', exact: true }).click();
	await expect.element(view.getByText('Skipped', { exact: true })).toBeVisible();
	await view.getByRole('button', { name: 'Edit Storage', exact: true }).click();
	await view.getByRole('button', { name: 'Remote', exact: true }).click();
	await expect.element(view.getByRole('button', { name: 'Local Recommended', exact: true })).toHaveAttribute('aria-pressed', 'false');
	await view.getByRole('button', { name: 'Review', exact: true }).click();
	await expect.poll(() => view.container.scrollWidth <= view.container.clientWidth).toBe(true);
	await view.getByRole('button', { name: 'Submit answers', exact: true }).click();
	const responses: Record<string, QuestionResponse> = {
		storage: { selected: ['remote'], text: 'Keep offline support.', skipped: false },
		tools: { selected: ['read', 'write'], text: 'Ask before deleting.', skipped: false },
		notes: { selected: [], text: 'First line\nSecond line', skipped: false },
		alternative: { selected: [], text: 'A custom alternative', skipped: false },
		optional: { selected: [], text: '', skipped: true },
		finish: { selected: [], text: 'Done', skipped: false }
	};
	expect(onanswer).toHaveBeenCalledExactlyOnceWith({ responses });
	await view.unmount();
	const history = await render(Questions, { record: { after: 1, questions, answers: [], responses } });
	await history.getByText('6 questions answered', { exact: false }).click();
	await expect.element(history.getByText('Answer: Remote\nKeep offline support.', { exact: true })).toBeVisible();
	await expect.element(history.getByText('Answer: First line\nSecond line', { exact: true })).toBeVisible();
	await expect.element(history.getByText('Answer: Skipped', { exact: true })).toBeVisible();
});

test('incomplete questions cannot be submitted and cancellation returns no answers', async () => {
	const onanswer = vi.fn();
	const view = await render(RequestCard, { request: { kind: 'question', id: 'questions', questions }, agent: 'Codex', disabled: false, onanswer });
	await view.getByRole('button', { name: 'Review', exact: true }).click();
	await expect.element(view.getByRole('button', { name: 'Submit answers' })).toBeDisabled();
	await view.getByRole('button', { name: 'Cancel questions' }).click();
	expect(onanswer).toHaveBeenCalledExactlyOnceWith({ allow: false });
});

test('disabled question cards preserve their inputs', async () => {
	const onanswer = vi.fn();
	const view = await render(RequestCard, { request: { kind: 'question', id: 'questions', questions }, agent: 'Claude', disabled: true, onanswer });
	for (const button of view.getByRole('button').elements().filter((node) => node.getAttribute('aria-label') !== 'Copy')) expect(button).toBeDisabled();
	await expect.element(view.getByRole('textbox', { name: 'Additional text (optional)' })).toBeDisabled();
	expect(onanswer).not.toHaveBeenCalled();
});
