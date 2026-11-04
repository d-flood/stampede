<script lang="ts">
	import { onDestroy, tick, untrack } from 'svelte';
	import { cubicOut } from 'svelte/easing';
	import { prefersReducedMotion } from 'svelte/motion';
	import { SvelteMap } from 'svelte/reactivity';
	import { slide } from 'svelte/transition';
	import { answered, answerText, loadDraft, normalized, saveDraft } from '#lib/chat/questions.ts';
	import type { Answer, Question, QuestionResponse } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';
	import Button from '#lib/ui/Button.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Text from '#lib/chat/Text.svelte';
	import Media from '#lib/chat/Media.svelte';
	import QuestionOption from '#lib/chat/QuestionOption.svelte';
	import CaretDown from '~icons/ph/caret-down';
	import CaretRight from '~icons/ph/caret-right';

	let { id, questions, disabled, onanswer }: { id: string; questions: Question[]; disabled: boolean; onanswer(answer: Answer): void } = $props();
	const items = $derived(questions.map(normalized));
	const responses = new SvelteMap<string, QuestionResponse>(Object.entries(untrack(() => loadDraft(id))));
	const isDone = (item: Question) => answered(responses.get(item.id));
	let open = $state(untrack(() => items.findIndex((item) => !isDone(item))));
	let shut = $state(false);
	let armed = $state(false);
	let root: HTMLElement;
	let scroller = $state<HTMLElement>();
	let moving: ReturnType<typeof setTimeout>;
	let disarming: ReturnType<typeof setTimeout>;
	const one = $derived(items.length === 1);
	const phone = $derived(form() === 'phone');
	const current = $derived(one ? items[0] : items[open]);
	const done = $derived(items.filter(isDone).length);
	const motion = $derived({ duration: prefersReducedMotion.current ? 0 : 200, easing: cubicOut });
	const sendLabel = $derived(one ? 'Send answer' : done === items.length || !done ? 'Send answers' : `Send ${done}, skip ${items.length - done}`);

	onDestroy(() => {
		clearTimeout(moving);
		clearTimeout(disarming);
	});

	export function focus() {
		root.focus();
	}

	function update(item: Question, change: Partial<QuestionResponse>) {
		responses.set(item.id, { selected: [], text: '', skipped: false, ...responses.get(item.id), ...change });
		saveDraft(id, Object.fromEntries(responses));
		armed = false;
	}

	function choose(item: Question, option: string) {
		const selected = responses.get(item.id)?.selected ?? [];
		update(item, { selected: item.type !== 'multiple' ? [option] : selected.includes(option) ? selected.filter((value) => value !== option) : [...selected, option] });
	}

	function clear(item: Question) {
		responses.delete(item.id);
		saveDraft(id, Object.fromEntries(responses));
		armed = false;
	}

	function following(from: number) {
		for (let step = 1; step <= items.length; step++) {
			const at = (from + step + items.length) % items.length;
			if (at !== from && !isDone(items[at])) return at;
		}
		return -1;
	}

	async function show(index: number) {
		clearTimeout(moving);
		open = index;
		armed = false;
		await tick();
		if (phone || index < 0) return;
		const panel = root.querySelector('li.open .panel');
		(panel?.querySelector<HTMLElement>('input:checked') ?? panel?.querySelector<HTMLElement>('input, textarea'))?.focus({ preventScroll: true });
	}

	async function advance() {
		const next = following(open);
		await show(next);
		if (next < 0 && !phone) root.querySelector<HTMLElement>('[data-send]')?.focus();
	}

	function pause() {
		clearTimeout(moving);
		moving = setTimeout(advance, prefersReducedMotion.current ? 0 : 180);
	}

	function clicked(item: Question, event: MouseEvent) {
		if (one || disabled || item.type !== 'single' || !event.detail || (event.target as Element).closest('a, button')) return;
		pause();
	}

	function reveal(row: HTMLElement) {
		const top = row.offsetTop;
		const bottom = top + row.offsetHeight;
		if (!scroller) return;
		const at = scroller.scrollTop;
		const height = scroller.clientHeight;
		const target = bottom - top > height || top < at ? top : bottom > at + height ? bottom - height : at;
		if (target !== at) scroller.scrollTo({ top: target, behavior: prefersReducedMotion.current ? 'auto' : 'smooth' });
	}

	function send() {
		if (disabled || !done) return;
		const given = items.map((item) => ({ item, response: isDone(item) ? responses.get(item.id)! : undefined, text: responses.get(item.id)?.text.trim() ?? '' }));
		if (questions[0].type) {
			onanswer({ responses: Object.fromEntries(given.map(({ item, response, text }) => [item.id, response ? { selected: response.selected, text, skipped: false } : { selected: [], text: '', skipped: true }])) });
		} else {
			onanswer({ answers: given.map(({ item, response, text }) => response ? [...item.options.filter((option) => response.selected.includes(option.id!)).map((option) => option.label), ...(text ? [text] : [])] : []) });
		}
	}

	function dismiss() {
		if (done && !armed) {
			armed = true;
			clearTimeout(disarming);
			disarming = setTimeout(() => armed = false, 4000);
		} else onanswer({ allow: false });
	}

	function keydown(event: KeyboardEvent) {
		const target = event.target as HTMLElement;
		const typing = target.matches('textarea');
		const option = !typing && !event.ctrlKey && !event.metaKey && !event.altKey && /^[1-9]$/.test(event.key) ? current?.options[Number(event.key) - 1] : undefined;
		if (option && !disabled) {
			event.preventDefault();
			choose(current, option.id!);
			if (!one && current.type === 'single') pause();
		} else if (event.key === 'Enter' && !event.shiftKey && !event.isComposing && !target.closest('a, button') && !(typing && phone)) {
			event.preventDefault();
			if (one) send();
			else advance();
		}
	}
</script>

{#snippet prompt(item: Question)}
	{@const response = responses.get(item.id)}
	{@const tiles = item.options.length > 1 && item.options.every((option) => option.image)}
	<Text item={{ kind: 'text', text: item.question, streaming: false, phase: 'final' }} />
	{#if item.image}<Media item={{ kind: 'media', ...item.image }} />{/if}
	{#if item.options.length}
		<div class="options" class:tiles role={item.type === 'multiple' ? 'group' : 'radiogroup'} aria-label={item.type === 'multiple' ? 'Choose any options' : 'Choose one option'}>
			{#each item.options as option, index}
				<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions (The input inside handles keys; this only tells a pointer pick apart from an arrow key.) -->
				<label class="option" onclick={(event) => clicked(item, event)}>
					<input type={item.type === 'multiple' ? 'checkbox' : 'radio'} name="{id}/{item.id}" checked={!!response?.selected.includes(option.id!)} {disabled} aria-label={option.recommended ? `${option.label}, recommended` : option.label} onchange={() => choose(item, option.id!)} />
					<div class="details"><QuestionOption {option} /></div>
					{#if !phone && !tiles && index < 9}<span class="key" aria-hidden="true">{index + 1}</span>{/if}
				</label>
			{/each}
		</div>
	{/if}
	<div class="note" class:aligned={item.options.length && !tiles}>
		<textarea rows={item.options.length ? 1 : 2} maxlength={16384} value={response?.text ?? ''} {disabled} placeholder={item.options.length ? 'Add a note or your own answer' : 'Your answer'} aria-label={item.options.length ? 'Add a note or your own answer' : 'Your answer'} oninput={(event) => update(item, { text: event.currentTarget.value })}></textarea>
		{#if !one && answered(response)}<button type="button" class="quiet" {disabled} onclick={() => clear(item)}>Clear answer</button>{/if}
	</div>
{/snippet}

<!-- svelte-ignore a11y_no_static_element_interactions (Number keys and Enter work from any control in the form.) -->
<div class="form" class:phone bind:this={root} tabindex="-1" onkeydown={keydown}>
	<header>
		<h2>{one ? items[0].header || 'Question' : `${items.length} questions`}</h2>
		{#if !one && !(phone && armed)}<span class="progress">{phone ? `${done} answered` : `${done} of ${items.length} answered`}</span>{/if}
		<button type="button" class="quiet" class:armed {disabled} onclick={dismiss}>{armed ? (done === 1 ? 'Discard 1 answer?' : `Discard ${done} answers?`) : 'Dismiss'}</button>
		<IconButton label={shut ? 'Show questions' : 'Hide questions'} aria-expanded={!shut} onclick={() => shut = !shut}><span class="chevron" class:shut><CaretDown /></span></IconButton>
	</header>
	{#if !shut}
		<div class="content" transition:slide={motion}>
			<div class="scroll" bind:this={scroller}>
				{#if one}
					<div class="panel single">{@render prompt(items[0])}</div>
				{:else}
					<ol>
						{#each items as item, index (item.id)}
							{@const response = responses.get(item.id)}
							<li class:open={index === open}>
								<button type="button" class="row" aria-expanded={index === open} onclick={() => show(index === open ? -1 : index)}>
									<span class="caret"><CaretRight /></span>
									<span class="title">{item.header}</span>
									{#if index !== open}<span class="value" class:none={!answered(response)}>{response && answered(response) ? answerText(item, response) : 'Not answered'}</span>{/if}
								</button>
								{#if index === open}
									<div class="panel" transition:slide={motion} onintroend={(event) => reveal(event.currentTarget.parentElement!)}>{@render prompt(item)}</div>
								{/if}
							</li>
						{/each}
					</ol>
				{/if}
			</div>
			<footer>
				{#if !one && open >= 0 && following(open) >= 0}<Button variant="ghost" {disabled} onclick={advance}>Next question</Button>{/if}
				<Button variant="primary" data-send disabled={disabled || !done} onclick={send}>{sendLabel}</Button>
			</footer>
		</div>
	{/if}
</div>

<style>
	.form { display: flex; flex-direction: column; flex: 1 1 auto; min-height: 0; outline: none; }
	header { flex: none; display: flex; align-items: center; gap: var(--space-2); min-height: calc(var(--target-min) + var(--space-4)); padding: var(--space-1) var(--space-2) var(--space-1) var(--space-4); }
	h2 { flex: 1; min-width: 0; margin: 0; color: var(--status-waiting); font-size: var(--font-size-md); font-weight: 600; overflow-wrap: anywhere; }
	.progress { flex: none; color: var(--muted-fg); font-size: var(--font-size-sm); }
	.quiet { flex: none; min-height: var(--target-min); padding: 0 var(--space-2); border: 0; border-radius: var(--radius-sm); background: none; color: var(--muted-fg); font: inherit; font-size: var(--font-size-sm); cursor: pointer; }
	.quiet:hover:not(:disabled) { background: var(--hover); color: var(--color-fg); }
	.quiet.armed { color: var(--color-fg); text-decoration: underline; }
	.chevron { display: inline-flex; transition: transform var(--dur-med) var(--ease-out); }
	.chevron.shut { transform: rotate(180deg); }
	.content { display: flex; flex-direction: column; flex: 1 1 auto; min-height: 0; }
	.scroll {
		--scroll-shadow: color-mix(in oklab, var(--ansi-0) 35%, transparent);
		position: relative;
		flex: 1 1 auto;
		min-height: 0;
		overflow-y: auto;
		border-top: var(--border-width) solid var(--border);
		background:
			linear-gradient(var(--surface-2) 40%, transparent) top / 100% 28px no-repeat local,
			linear-gradient(transparent, var(--surface-2) 60%) bottom / 100% 28px no-repeat local,
			linear-gradient(var(--scroll-shadow), transparent) top / 100% 10px no-repeat scroll,
			linear-gradient(transparent, var(--scroll-shadow)) bottom / 100% 10px no-repeat scroll;
	}
	footer { flex: none; display: flex; flex-wrap: wrap; gap: var(--space-2); padding: var(--space-3) var(--space-4); border-top: var(--border-width) solid var(--border); }
	footer > :global(button:last-child) { margin-left: auto; }
	ol { margin: 0; padding: 0; list-style: none; }
	li + li { border-top: var(--border-width) solid var(--border); }
	.row { display: grid; grid-template-columns: 1em 8.5rem minmax(0, 1fr); align-items: center; column-gap: var(--space-3); width: 100%; min-height: var(--target-min); padding: var(--space-3) var(--space-4); border: 0; background: none; color: inherit; font: inherit; text-align: left; cursor: pointer; }
	.row:hover { background: var(--hover); }
	.row:focus-visible, .quiet:focus-visible { outline: 2px solid color-mix(in oklab, var(--status-working) 70%, transparent); outline-offset: -2px; }
	.caret { display: inline-flex; color: var(--muted-fg); transition: transform var(--dur-med) var(--ease-out); }
	li.open .caret { transform: rotate(90deg); }
	li.open .row { grid-template-columns: 1em minmax(0, 1fr); }
	.title { font-weight: 600; overflow-wrap: anywhere; }
	.value { min-width: 0; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
	.none { color: var(--muted-fg); }
	.panel { display: flex; flex-direction: column; gap: var(--space-3); min-width: 0; padding: 0 var(--space-4) var(--space-4) calc(var(--space-4) + 1em + var(--space-3)); }
	.panel.single { padding: var(--space-3) var(--space-4) var(--space-4); }
	.panel :global(figure) { flex: none; border: 0; border-radius: var(--radius-md); background: none; }
	.panel :global(figcaption) { padding: 0 0 var(--space-1); background: none; color: var(--muted-fg); font-size: var(--font-size-sm); }
	.panel :global(figure img) { max-height: 20dvh; object-position: left; }
	.panel :global(.text > :last-child) { margin-bottom: 0; }
	.options { display: flex; flex-direction: column; gap: var(--space-1); }
	.option { display: grid; grid-template-columns: auto minmax(0, 1fr) auto; column-gap: var(--space-3); margin-inline: calc(var(--space-2) * -1); padding: var(--space-2); border-radius: var(--radius-md); cursor: pointer; transition: background var(--dur-fast); }
	.option:hover { background: var(--hover); }
	.details { display: flex; flex-direction: column; gap: var(--space-1); min-width: 0; }
	.tiles { display: grid; grid-template-columns: repeat(auto-fit, minmax(10rem, 1fr)); gap: var(--space-2); }
	.tiles .option { margin: 0; grid-template-columns: auto minmax(0, 1fr); row-gap: var(--space-2); }
	.tiles .details { display: contents; }
	.tiles :global(strong > span) { display: block; margin-left: 0; }
	.tiles :global(figure) { position: relative; grid-column: 1 / -1; }
	.tiles :global(figcaption) { position: absolute; top: var(--space-1); right: var(--space-1); padding: 0 var(--space-2); border-radius: var(--radius-sm); background: var(--surface-2); }
	.tiles :global(figcaption > span) { display: none; }
	.tiles :global(figure img) { width: 100%; object-position: center; }
	.tiles .option:has(input:checked) :global(figure) { outline: 2px solid var(--status-waiting); outline-offset: -2px; }
	input { appearance: none; display: grid; place-content: center; flex: none; width: 16px; height: 16px; margin: 0.2em 0 0; border: 1.5px solid var(--muted-fg); background: none; cursor: pointer; transition: border-color var(--dur-fast), background var(--dur-fast); }
	input[type='radio'] { border-radius: 50%; }
	input[type='checkbox'] { border-radius: 4px; }
	input:checked { border-color: var(--status-waiting); }
	input[type='radio']:checked { background: radial-gradient(circle, var(--status-waiting) 42%, transparent 48%); }
	input[type='checkbox']:checked { background: var(--status-waiting); }
	input[type='checkbox']:checked::before { content: ''; width: 10px; height: 10px; background: var(--color-bg); clip-path: polygon(14% 44%, 0 65%, 40% 100%, 100% 16%, 82% 0, 38% 65%); }
	input:focus-visible { outline: 2px solid color-mix(in oklab, var(--status-working) 70%, transparent); outline-offset: 2px; }
	input:disabled { opacity: 0.5; cursor: default; }
	.key { min-width: 1ch; color: var(--muted-fg); font-family: var(--font-mono); font-size: var(--font-size-sm); opacity: 0; transition: opacity var(--dur-fast); }
	.form:focus-within .key { opacity: 1; }
	.note { display: flex; align-items: flex-end; gap: var(--space-2); }
	.note.aligned { padding-left: calc(16px + var(--space-3)); }
	textarea { flex: 1; min-width: 0; min-height: var(--target-min); max-height: 12lh; padding: var(--space-1) 0; border: 0; border-bottom: var(--border-width) solid var(--border-strong); border-radius: 0; background: none; color: inherit; font: inherit; line-height: 1.4; resize: none; field-sizing: content; transition: border-color var(--dur-fast); }
	textarea::placeholder { color: var(--muted-fg); }
	textarea:focus { outline: none; border-color: var(--status-working); box-shadow: 0 1px 0 var(--status-working); }
	.phone h2 { font-size: var(--font-size-lg); }
	.phone .row { grid-template-columns: 1em minmax(0, 1fr); min-height: 44px; }
	.phone .value { grid-column: 2; white-space: normal; display: -webkit-box; -webkit-box-orient: vertical; -webkit-line-clamp: 2; line-clamp: 2; }
	.phone .panel { padding-left: var(--space-4); }
	.phone .option { padding-block: var(--space-3); }
	.phone .quiet { min-height: 44px; }
	.phone header :global(button) { min-width: 44px; min-height: 44px; }
	.phone textarea { min-height: 44px; padding-block: var(--space-3); font-size: var(--font-size-lg); }
	.phone footer > :global(button) { flex: 1; min-height: 44px; font-size: var(--font-size-lg); }
	.phone footer > :global(button:last-child) { margin-left: 0; }
</style>
