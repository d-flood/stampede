<script lang="ts">
	import Code from '#lib/chat/Code.svelte';
	import { icons } from '#lib/chat/icons.ts';
	import type { Answer, Request } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';
	import Button from '#lib/ui/Button.svelte';
	import Text from '#lib/chat/Text.svelte';
	import QuestionOption from '#lib/chat/QuestionOption.svelte';
	import QuestionForm from '#lib/chat/QuestionForm.svelte';
	import CaretRight from '~icons/ph/caret-right';

	let { request, agent, asker, disabled, onanswer }: { request: Request; agent: string; asker?: string; disabled: boolean; onanswer(answer: Answer): void } = $props();
	let card: HTMLElement;
	let picking = $state(false);
	let scope = $state<'chat' | 'always'>('chat');
	let stops = $state<number[]>([]);
	let step = $state(0);
	let answers = $state<string[][]>([]);
	let selected = $state<string[]>([]);
	let other = $state('');
	const question = $derived(request.kind === 'question' ? request.questions[step] : undefined);
	const title = $derived(request.kind === 'command' ? `${agent} wants to run a command` : request.kind === 'edit' ? `${agent} wants to edit a file` : request.kind === 'plan' ? 'Proposed plan' : request.kind === 'question' ? question?.header : request.title);

	export function focus() {
		card.focus();
	}

	function quote(word: string) {
		if (/^[\w@%+=:,./-]+$/.test(word)) return word;
		if (word.includes("'") && !/["$`\\]/.test(word)) return `"${word}"`;
		return `'${word.replaceAll("'", `'\\''`)}'`;
	}

	function stop(index: number, length: number) {
		if (request.kind !== 'command') return length;
		return scope === 'chat' && !request.sessionPrefixes ? length : stops[index] ?? request.suggestion[index]?.length ?? length;
	}

	function choose(label: string) {
		selected = question?.multiSelect ? selected.includes(label) ? selected.filter((value) => value !== label) : [...selected, label] : [label];
	}

	function next(skip = false) {
		const chosen = skip ? [] : [...selected, ...(other.trim() ? [other.trim()] : [])];
		const result = [...answers, chosen];
		if (request.kind !== 'question' || step + 1 >= request.questions.length) onanswer({ answers: result });
		else {
			answers = result;
			step += 1;
			selected = [];
			other = '';
		}
	}

	function allowRules() {
		if (request.kind !== 'command') return;
		onanswer({ allow: true, rule: request.words.map((words, index) => ({ prefix: words.slice(0, stop(index, words.length)), scope })) });
	}
</script>

<section aria-label={request.kind === 'question' ? 'Question' : request.kind === 'plan' ? 'Plan' : 'Approval'} class="card" class:phone={form() === 'phone'} tabindex="-1" bind:this={card}>
	{#if asker}<div class="asker"><icons.subagent />Asked by {asker}</div>{/if}
	{#if request.kind !== 'question'}<h2>{title}</h2>{/if}
	{#if request.kind === 'question' && request.questions[0]?.type}
		<QuestionForm questions={request.questions} {disabled} {onanswer} />
	{:else if request.kind === 'question' && question}
		<details open>
			<summary><span class="caret"><CaretRight /></span><h2>{title}</h2><span class="progress">Question {step + 1} of {request.questions.length}</span></summary>
			<div class="question-body">
				<div>{question.question}</div>
				<div class="options">
					{#each question.options as option}
						<button class="option" aria-pressed={selected.includes(option.label)} {disabled} onclick={() => choose(option.label)}>
							<QuestionOption {option} />
						</button>
					{/each}
					<label>Other…<input aria-label="Other answer" bind:value={other} {disabled} /></label>
				</div>
				<div class="actions">
					<Button variant="primary" disabled={disabled || (!selected.length && !other.trim())} onclick={() => next()}>{step + 1 === request.questions.length ? 'Submit' : 'Next'}</Button>
					<Button {disabled} onclick={() => next(true)}>Skip</Button>
				</div>
			</div>
		</details>
	{:else if request.kind === 'plan'}
		<div class="code"><Text item={{ kind: 'text', text: request.plan, streaming: false, phase: 'final' }} /></div>
		<div class="actions">
			<Button variant="primary" {disabled} onclick={() => onanswer({ allow: true })}>Approve</Button>
			<Button {disabled} onclick={() => onanswer({ allow: false })}>Keep planning</Button>
		</div>
	{:else}
		{#if request.kind === 'command'}
			<pre class="code"><Code code={request.words.map((words) => words.map(quote).join(' ')).join('\n')} path="command.sh" /></pre>
			{#if picking}
				<label>Rule scope <select aria-label="Rule scope" bind:value={scope} {disabled}><option value="chat">This Chat</option><option value="always">Always</option></select></label>
				{#if scope === 'always' && !request.sessionPrefixes && request.words.length > 1}<div class="progress">Additional compound rules take effect after Codex restarts.</div>{/if}
				{#if scope === 'chat' && !request.sessionPrefixes}<div class="progress">This Chat allows the exact command. Prefix rules require Always.</div>{/if}
				{#each request.words as words, index}
					<div class="words" aria-label="Command part {index + 1}">
						{#each words as word, at}
							<button class="word" aria-pressed={at < stop(index, words.length)} disabled={disabled || (scope === 'chat' && !request.sessionPrefixes)} onclick={() => (stops[index] = at + 1)}>{word}</button>
						{/each}
					</div>
					<output>{words.slice(0, stop(index, words.length)).map(quote).join(' ')}{stop(index, words.length) < words.length ? ' *' : ''}</output>
				{/each}
			{/if}
		{:else if request.kind === 'edit'}
			<div class="path">{request.path}</div>
			<div class="code"><Code code={request.diff} path={request.path} diff /></div>
		{:else if request.kind === 'other'}
			<pre class="code"><Code code={request.detail} path="arguments.json" /></pre>
		{/if}
		<div class="actions">
			{#if picking}
				<Button variant="primary" {disabled} onclick={allowRules}>Allow rule{request.kind === 'command' && request.words.length > 1 ? 's' : ''}</Button>
				<Button {disabled} onclick={() => (picking = false)}>Back</Button>
			{:else}
				<Button variant="primary" {disabled} onclick={() => onanswer({ allow: true })}>Allow</Button>
				{#if request.kind === 'command'}<Button {disabled} onclick={() => (picking = true)}>Always allow</Button>{/if}
			{/if}
			<Button variant="danger" {disabled} onclick={() => onanswer({ allow: false })}>Deny</Button>
		</div>
	{/if}
</section>

<style>
	.question-body { display: flex; flex-direction: column; gap: var(--space-3); padding-top: var(--space-3); }
	summary { display: flex; align-items: center; flex-wrap: wrap; gap: var(--space-2); min-height: var(--target-min); list-style: none; cursor: pointer; }
	summary::-webkit-details-marker { display: none; }
	.caret { display: inline-flex; flex: none; width: 1em; color: var(--status-waiting); transition: transform var(--dur-fast); }
	details[open] > summary > .caret { transform: rotate(90deg); }
	summary h2 { flex: 1; overflow-wrap: anywhere; }
	.options { display: flex; flex-direction: column; gap: var(--space-2); }
	.option { display: flex; flex-direction: column; align-items: flex-start; gap: var(--space-1); text-align: left; }
	.progress { color: var(--muted-fg); font-size: var(--font-size-sm); }
	.asker { display: flex; align-items: center; gap: var(--space-1); color: var(--muted-fg); font-size: var(--font-size-sm); overflow-wrap: anywhere; }
	.asker :global(svg) { flex: none; }
	.option, .word, input, select { padding: var(--space-2) var(--space-3); min-height: var(--target-min); border: var(--border-width) solid var(--border); border-radius: var(--radius-md); background: var(--color-bg); color: var(--color-fg); font: inherit; }
	button[aria-pressed='true'] { border-color: var(--status-waiting); background: var(--hover); }
	.word { max-width: 100%; overflow-wrap: anywhere; }
	.words { display: flex; gap: var(--space-1); flex-wrap: wrap; }
	label { display: flex; align-items: center; gap: var(--space-2); flex-wrap: wrap; }
	input { flex: 1; min-width: 0; }
	output { font-family: var(--font-mono); overflow-wrap: anywhere; }
	.card {
		max-height: 55dvh;
		overflow-y: auto;
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		padding: var(--space-4);
		border: var(--border-width) solid var(--status-waiting);
		border-radius: var(--radius-lg);
		background: var(--surface-2);
	}

	h2 {
		margin: 0;
		color: var(--status-waiting);
		font-size: var(--font-size-md);
		font-weight: 600;
	}

	.path {
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		color: var(--muted-fg);
		overflow-wrap: anywhere;
	}

	.code {
		max-height: 30vh;
		margin: 0;
		padding: var(--space-2) var(--space-3);
		overflow: auto;
		border-radius: var(--radius-md);
		background: var(--color-bg);
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		overflow-wrap: anywhere;
	}

	pre.code,
	.code :global(.code-line) {
		white-space: pre-wrap;
	}

	.code :global(.added) {
		background: color-mix(in oklab, var(--git-added) 14%, transparent);
	}

	.code :global(.removed) {
		background: color-mix(in oklab, var(--git-deleted) 14%, transparent);
	}

	.code :global(.hunk) {
		color: var(--muted-fg);
	}

	.actions {
		display: flex;
		gap: var(--space-2);
	}

	.actions > :global(button:first-child) {
		font-weight: 600;
	}

	.phone h2 {
		font-size: var(--font-size-lg);
	}

	.phone .code {
		font-size: var(--font-size-md);
	}

	.phone .actions {
		flex-wrap: wrap;
	}

	.phone .actions > :global(button) {
		flex: 1;
		min-height: 44px;
		font-size: var(--font-size-lg);
	}

	.phone .actions > :global(button:first-child) {
		flex-basis: 100%;
		min-height: 46px;
	}
</style>
