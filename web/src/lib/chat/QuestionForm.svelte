<script lang="ts">
	import { SvelteMap } from 'svelte/reactivity';
	import type { Answer, Question, QuestionResponse } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';
	import Button from '#lib/ui/Button.svelte';
	import TextArea from '#lib/ui/TextArea.svelte';
	import Text from '#lib/chat/Text.svelte';
	import Media from '#lib/chat/Media.svelte';
	import QuestionOption from '#lib/chat/QuestionOption.svelte';
	import CaretRight from '~icons/ph/caret-right';

	let { questions, disabled, onanswer }: { questions: Question[]; disabled: boolean; onanswer(answer: Answer): void } = $props();
	let step = $state(0);
	let reviewing = $state(false);
	const responses = new SvelteMap<string, QuestionResponse>();
	const question = $derived(questions[step]);
	const response = $derived(responses.get(question.id) ?? { selected: [], text: '', skipped: false });
	const ready = $derived(questions.every((question) => complete(responses.get(question.id))));

	function complete(answer?: QuestionResponse) {
		return !!answer && (answer.skipped || !!answer.selected.length || !!answer.text.trim());
	}

	function choose(id: string) {
		const selected = response.selected.includes(id) ? response.selected.filter((value) => value !== id)
			: question.type === 'multiple' ? [...response.selected, id] : [id];
		responses.set(question.id, { ...response, selected, skipped: false });
	}

	function text(value: string) {
		responses.set(question.id, { ...response, text: value, skipped: false });
	}

	function advance(skip = false) {
		if (skip) responses.set(question.id, { selected: [], text: '', skipped: true });
		if (step + 1 < questions.length) step += 1;
		else reviewing = true;
	}

	function edit(index: number) {
		step = index;
		reviewing = false;
	}

	function summary(question: Question) {
		const answer = responses.get(question.id);
		if (!answer || !complete(answer)) return 'Unanswered';
		if (answer.skipped) return 'Skipped';
		return [...question.options.filter((option) => answer.selected.includes(option.id ?? '')).map((option) => option.label), answer.text].filter(Boolean).join('\n');
	}
</script>

<details open class:phone={form() === 'phone'}>
	<summary class="heading"><span class="caret"><CaretRight /></span><h2>{reviewing ? 'Review answers' : question.header}</h2><span class="progress">{reviewing ? `${questions.length} questions` : `Question ${step + 1} of ${questions.length}`}</span></summary>
	<div class="body">
		{#if !reviewing}
			<div class="layout">
				{#if form() !== 'phone' && questions.length > 1}
					<nav aria-label="Questions">
						{#each questions as item, index}
							<button class="nav-item" aria-current={index === step ? 'step' : undefined} {disabled} onclick={() => edit(index)}><span>{index + 1}. {item.header}</span><span class="progress">{responses.get(item.id)?.skipped ? 'Skipped' : complete(responses.get(item.id)) ? 'Answered' : 'Unanswered'}</span></button>
						{/each}
					</nav>
				{/if}
				<div class="prompt">
					<Text item={{ kind: 'text', text: question.question, streaming: false, phase: 'final' }} />
					{#if question.image}<Media item={{ kind: 'media', ...question.image }} />{/if}
					{#if question.type !== 'text'}
						<div class="options" role="group" aria-label={question.type === 'multiple' ? 'Choose any options' : 'Choose one option'}>
							{#each question.options as option}
								<div class="option" class:selected={response.selected.includes(option.id ?? '')}>
									<button type="button" class="choice" aria-pressed={response.selected.includes(option.id ?? '')} {disabled} onclick={() => choose(option.id!)}>{#if response.selected.includes(option.id ?? '')}<span aria-hidden="true">✓</span>{/if}{option.label}{#if option.recommended}<span class="recommended">Recommended</span>{/if}</button>
									<div class="description"><QuestionOption {option} rich showLabel={false} /></div>
								</div>
							{/each}
						</div>
					{/if}
					<label>{question.type === 'text' ? 'Answer' : 'Additional text (optional)'}<TextArea rows={4} maxlength={16384} value={response.text} {disabled} oninput={(event) => text(event.currentTarget.value)} /></label>
				</div>
			</div>
			<div class="actions">
				<Button variant="primary" disabled={disabled || !complete(response)} onclick={() => advance()}>{step + 1 === questions.length ? 'Review answers' : 'Next'}</Button>
				<Button disabled={disabled || step === 0} onclick={() => step -= 1}>Back</Button>
				<Button {disabled} onclick={() => advance(true)}>Skip</Button>
				<Button {disabled} onclick={() => reviewing = true}>Review</Button>
			</div>
		{:else}
			<ol class="review" aria-label="Answer review">
				{#each questions as item, index}
					<li><div><h3>{item.header}</h3><p>{summary(item)}</p></div><Button {disabled} aria-label={`Edit ${item.header}`} onclick={() => edit(index)}>Edit</Button></li>
				{/each}
			</ol>
			{#if !ready}<p class="progress">Answer or skip every question before submitting.</p>{/if}
			<div class="actions">
				<Button variant="primary" disabled={disabled || !ready} onclick={() => onanswer({ responses: Object.fromEntries(responses) })}>Submit answers</Button>
				<Button {disabled} onclick={() => reviewing = false}>Back</Button>
			</div>
		{/if}
		<Button {disabled} onclick={() => onanswer({ allow: false })}>Cancel questions</Button>
	</div>
</details>

<style>
	details, .prompt, .description { min-width: 0; }
	.heading { display: flex; align-items: center; flex-wrap: wrap; gap: var(--space-2); min-height: var(--target-min); list-style: none; cursor: pointer; }
	.heading::-webkit-details-marker { display: none; }
	.caret { display: inline-flex; flex: none; width: 1em; color: var(--status-waiting); transition: transform var(--dur-fast); }
	details[open] > .heading > .caret { transform: rotate(90deg); }
	h2, h3, p { margin: 0; overflow-wrap: anywhere; }
	h2 { flex: 1; color: var(--status-waiting); font-size: var(--font-size-md); font-weight: 600; }
	h3 { font-size: var(--font-size-md); }
	.progress { color: var(--muted-fg); font-size: var(--font-size-sm); }
	.body, .prompt, .options, nav, label { display: flex; flex-direction: column; gap: var(--space-3); }
	.body { padding-top: var(--space-3); }
	.layout { display: flex; gap: var(--space-4); align-items: flex-start; }
	nav { position: sticky; top: 0; flex: 0 0 30%; max-width: 15rem; max-height: 30dvh; overflow-y: auto; gap: var(--space-1); }
	.prompt { flex: 1; }
	.prompt :global(figure img) { max-height: 24dvh; }
	.nav-item, .choice { border: 0; background: none; color: inherit; font: inherit; text-align: left; cursor: pointer; min-height: var(--target-min); padding: var(--space-2) var(--space-3); overflow-wrap: anywhere; }
	.nav-item { display: flex; flex-direction: column; border-radius: var(--radius-md); }
	.nav-item[aria-current='step'] { background: var(--hover); color: var(--status-waiting); }
	.option { border: var(--border-width) solid var(--border); border-radius: var(--radius-md); overflow: hidden; }
	.option.selected { border-color: var(--status-waiting); background: var(--hover); }
	.choice { display: flex; align-items: baseline; flex-wrap: wrap; gap: var(--space-2); width: 100%; font-weight: 600; }
	.recommended { font-weight: 400; color: var(--status-waiting); font-size: var(--font-size-sm); }
	.description { display: flex; flex-direction: column; gap: var(--space-1); padding: 0 var(--space-3) var(--space-2); }
	.actions { display: flex; flex-wrap: wrap; gap: var(--space-2); }
	.review { display: flex; flex-direction: column; gap: var(--space-3); margin: 0; padding: 0; list-style: none; }
	.review li { display: flex; align-items: flex-start; justify-content: space-between; gap: var(--space-3); padding-bottom: var(--space-3); border-bottom: var(--border-width) solid var(--border); }
	.review li > div { min-width: 0; }
	.review p { margin-top: var(--space-1); white-space: pre-wrap; }
	.phone h2 { font-size: var(--font-size-lg); }
	.phone .actions > :global(button) { flex: 1; min-height: 44px; }
	.phone .actions > :global(button:first-child) { flex-basis: 100%; }
</style>
