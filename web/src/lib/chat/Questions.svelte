<script lang="ts">
	import Text from '#lib/chat/Text.svelte';
	import Media from '#lib/chat/Media.svelte';
	import QuestionOption from '#lib/chat/QuestionOption.svelte';
	import { answered, answerText, normalized } from '#lib/chat/questions.ts';
	import type { AnsweredQuestions, QuestionResponse } from '#lib/device.svelte.ts';
	import Caret from '#lib/ui/Caret.svelte';
	import Check from '~icons/ph/check';

	let { record }: { record: AnsweredQuestions } = $props();

	const entries = $derived(record.questions.map((question, index) => {
		const item = normalized(question, index);
		const given = record.answers[index] ?? [];
		const labels = item.options.map((option) => option.label);
		const response: QuestionResponse | undefined = question.type ? record.responses?.[question.id] : record.cancelled ? undefined : {
			selected: item.options.filter((option) => given.includes(option.label)).map((option) => option.id!),
			text: given.filter((answer) => !labels.includes(answer)).join(', '),
			skipped: !given.length
		};
		return { item, response: answered(response) ? response : undefined };
	}));
	const total = $derived(entries.length);
	const count = $derived(entries.filter(({ response }) => response).length);
	const noun = $derived(total === 1 ? '1 question' : `${total} questions`);
	const title = $derived(record.cancelled && !count ? `${noun} ${record.instruction ? 'not answered' : 'dismissed'}` : count === total ? `${noun} answered` : `${count} of ${total} answered`);
	const line = $derived(record.cancelled && !count
		? entries.map(({ item }) => item.header).filter(Boolean).join(', ')
		: entries.map(({ item, response }) => {
			const text = response ? answerText(item, response) : 'skipped';
			return total > 1 && item.header ? `${item.header}: ${text}` : text;
		}).join(' · '));
</script>

<details aria-label={count ? 'Answered questions' : 'Unanswered questions'}>
	<summary><Caret /><span class="count">{title}</span><span class="line">{line}</span></summary>
	<div class="questions">
		{#each entries as { item, response }}
			<section>
				{#if item.header}<h3>{item.header}</h3>{/if}
				<Text item={{ kind: 'text', text: item.question, streaming: false, phase: 'final' }} />
				{#if item.image}<Media item={{ kind: 'media', ...item.image }} />{/if}
				{#if item.options.length}
					<div class="options">
						{#each item.options as option}
							{@const selected = !!response?.selected.includes(option.id!)}
							<div class="option" class:selected>
								<span class="mark">{#if selected}<Check aria-label="Selected" />{/if}</span>
								<div class="details"><QuestionOption {option} /></div>
							</div>
						{/each}
					</div>
				{/if}
				{#if response?.text.trim()}<p class="answer">{response.text.trim()}</p>
				{:else if !response}<p class="unanswered">{record.cancelled ? 'Not answered' : 'Skipped'}</p>{/if}
			</section>
		{/each}
	</div>
</details>
{#each record.images ?? [] as image}<Media item={{ kind: 'media', ...image }} />{/each}
{#if record.instruction}<div class="instruction"><Text item={{ kind: 'text', text: record.instruction, streaming: false, phase: 'final' }} /></div>{/if}

<style>
	details { min-width: 0; }
	summary { display: flex; align-items: center; gap: var(--space-2); min-height: var(--target-min); color: var(--muted-fg); font-size: var(--font-size-sm); list-style: none; cursor: pointer; }
	summary::-webkit-details-marker { display: none; }
	summary:hover { color: var(--color-fg); }
	.count { flex: none; color: var(--color-fg); font-weight: 600; }
	.line { flex: 1; min-width: 0; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
	details[open] .line { display: none; }
	.questions { display: flex; flex-direction: column; min-width: 0; padding-left: calc(1em + var(--space-2)); }
	section { display: flex; flex-direction: column; gap: var(--space-2); min-width: 0; padding-block: var(--space-3); }
	section + section { border-top: var(--border-width) solid var(--border); }
	h3, p { margin: 0; }
	h3 { font-size: var(--font-size-md); font-weight: 600; }
	.options { display: flex; flex-direction: column; gap: var(--space-2); }
	.option { display: grid; grid-template-columns: 1em minmax(0, 1fr); column-gap: var(--space-2); color: var(--muted-fg); }
	.option.selected { color: var(--color-fg); }
	.mark { display: inline-flex; justify-content: center; padding-top: 0.2em; color: var(--status-done); }
	.details { display: flex; flex-direction: column; gap: var(--space-1); min-width: 0; }
	section :global(.text > :last-child) { margin-bottom: 0; }
	.answer { white-space: pre-wrap; overflow-wrap: anywhere; }
	.unanswered { color: var(--muted-fg); font-size: var(--font-size-sm); }
	.instruction { width: fit-content; max-width: 85%; margin: var(--space-2) 0 0 auto; padding: var(--space-3) var(--space-4); border: var(--border-width) solid var(--border); border-radius: var(--radius-lg) var(--radius-lg) var(--radius-sm) var(--radius-lg); background: var(--surface-2); }
</style>
