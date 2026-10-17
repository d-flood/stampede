<script lang="ts">
	import QuestionOption from '#lib/chat/QuestionOption.svelte';
	import type { AnsweredQuestions } from '#lib/device.svelte.ts';
	import CaretRight from '~icons/ph/caret-right';

	let { record }: { record: AnsweredQuestions } = $props();
</script>

<details aria-label="Answered questions">
	<summary><span class="caret"><CaretRight /></span>{record.questions.length} {record.questions.length === 1 ? 'question' : 'questions'} answered<span>{record.questions.map((question) => question.header).join(', ')}</span></summary>
	<div class="questions">
		{#each record.questions as question, index}
			<section>
				<h3>{question.header}</h3>
				<div class="question">{question.question}</div>
				<div class="options">
					{#each question.options as option}
						<div class="option" class:selected={record.answers[index]?.includes(option.label)}><QuestionOption {option} /></div>
					{/each}
				</div>
				<p class="answer">Answer: {record.answers[index]?.length ? record.answers[index].join(', ') : 'Skipped'}</p>
			</section>
		{/each}
	</div>
</details>

<style>
	details { min-width: 0; }
	summary { display: flex; align-items: center; flex-wrap: wrap; gap: var(--space-2); min-height: var(--target-min); color: var(--muted-fg); font-size: var(--font-size-sm); list-style: none; cursor: pointer; }
	summary::-webkit-details-marker { display: none; }
	.caret { display: inline-flex; flex: none; width: 1em; transition: transform var(--dur-fast); }
	details[open] > summary > .caret { transform: rotate(90deg); }
	summary:hover { color: var(--color-fg); }
	summary span { overflow-wrap: anywhere; }
	.questions, section, .options, .option { display: flex; flex-direction: column; gap: var(--space-3); min-width: 0; }
	.questions { padding-top: var(--space-2); }
	section { padding: var(--space-4); border: var(--border-width) solid var(--border); border-radius: var(--radius-lg); background: var(--surface-2); }
	h3, p { margin: 0; }
	h3 { font-size: var(--font-size-md); }
	.question, .answer { white-space: pre-wrap; overflow-wrap: anywhere; }
	.option { align-items: flex-start; gap: var(--space-1); padding: var(--space-2) var(--space-3); border: var(--border-width) solid var(--border); border-radius: var(--radius-md); }
	.selected { border-color: var(--status-done); background: var(--hover); }
	.answer { font-weight: 600; }
</style>
