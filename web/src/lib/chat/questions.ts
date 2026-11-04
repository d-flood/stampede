import type { Answer, Question, QuestionResponse, Request } from '#lib/device.svelte.ts';

const key = (request: string) => `question:${request}`;

export function loadDraft(request: string): Record<string, QuestionResponse> {
	return JSON.parse(localStorage.getItem(key(request)) ?? '{}');
}

export function saveDraft(request: string, responses: Record<string, QuestionResponse>) {
	localStorage.setItem(key(request), JSON.stringify(responses));
}

export function clearDraft(request: string) {
	localStorage.removeItem(key(request));
}

export function answered(response?: QuestionResponse) {
	return !!response && !response.skipped && (response.selected.length > 0 || !!response.text.trim());
}

export function normalized(question: Question, index: number): Question {
	return {
		...question,
		id: question.id || String(index),
		type: question.type ?? (!question.options.length ? 'text' : question.multiSelect ? 'multiple' : 'single'),
		options: question.options.map((option) => ({ ...option, id: option.id || option.label }))
	};
}

export function answerText(question: Question, response: QuestionResponse) {
	const labels = question.options.filter((option) => response.selected.includes(option.id!)).map((option) => option.label).join(', ');
	return [labels, response.text.trim()].filter(Boolean).join('. ');
}

export function redirect(request: Request, instead: string): Answer {
	if (request.kind !== 'question' || !request.questions[0]?.type) return { allow: false, instead };
	const given = Object.entries(loadDraft(request.id)).filter(([, response]) => answered(response));
	return { allow: false, instead, responses: Object.fromEntries(given.map(([id, response]) => [id, { ...response, text: response.text.trim() }])) };
}
