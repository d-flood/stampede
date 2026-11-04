<script lang="ts">
	import MarkdownEditor from '#lib/editor/MarkdownEditor.svelte';
	import { form } from '#lib/form.ts';
	import { sendReview, type PrReview, type ReviewEvent } from '#lib/pr-reviews.ts';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';

	type Props = { projectId: string; review: PrReview; open: boolean };

	let { projectId, review, open = $bindable() }: Props = $props();

	const events: { event: ReviewEvent; label: string; hint: string }[] = [
		{ event: 'COMMENT', label: 'Comment', hint: 'Feedback without approving.' },
		{ event: 'APPROVE', label: 'Approve', hint: 'Approve merging these changes.' },
		{ event: 'REQUEST_CHANGES', label: 'Request changes', hint: 'Feedback to address before merging.' }
	];

	let summary = $state('');
	let event = $state<ReviewEvent>('COMMENT');
	let pending = $state(false);
	let error = $state<string>();
	const offered = $derived(review.mine ? events.filter((choice) => choice.event === 'COMMENT') : events);
	const empty = $derived(!review.drafts.length && !summary.trim());

	async function send() {
		if (pending || empty) return;
		pending = true;
		error = undefined;
		try {
			await sendReview(projectId, review.number, summary, review.mine ? 'COMMENT' : event);
			open = false;
			summary = '';
			event = 'COMMENT';
		} catch (cause) {
			error = cause instanceof Error ? cause.message : String(cause);
		} finally {
			pending = false;
		}
	}
</script>

<Dialog bind:open wide full={form() === 'phone'} title="Send review of #{review.number}">
	{#if open}
		<div class="body">
			<section aria-labelledby="send-drafts">
				<h3 id="send-drafts">
					Draft comments <span class="count">{review.drafts.length}</span>
				</h3>
				{#if review.drafts.length}
					<ul aria-label="Draft comments to send">
						{#each review.drafts as draft (draft.id)}
							<li>
								<span class="where">{draft.path}:{draft.startLine ? `${draft.startLine}–${draft.line}` : draft.line}</span>
								{#if draft.side === 'target'}<span class="side">Target</span>{/if}
								<span class="line">{draft.body.trim().split('\n')[0]}</span>
							</li>
						{/each}
					</ul>
				{:else}
					<p class="none">None. The summary is sent on its own.</p>
				{/if}
			</section>
			<section class="summary">
				<h3>Summary</h3>
				<MarkdownEditor bind:text={summary} label="Review summary" />
			</section>
			<fieldset>
				<legend>Review as</legend>
				{#each offered as choice (choice.event)}
					<label class="choice">
						<input type="radio" name="event" value={choice.event} bind:group={event} disabled={pending} />
						<span class="label">{choice.label}</span>
						<span class="hint">{choice.hint}</span>
					</label>
				{/each}
				{#if review.mine}<p class="none">GitHub allows only comments on your own pull request.</p>{/if}
			</fieldset>
			{#if error}<p class="error" role="alert">{error}</p>{/if}
		</div>
	{/if}
	{#snippet actions()}
		<Button disabled={pending} onclick={() => (open = false)}>Cancel</Button>
		<Button variant="primary" disabled={pending || empty} onclick={send}>{pending ? 'Sending…' : 'Send'}</Button>
	{/snippet}
</Dialog>

<style>
	.body {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: var(--space-4);
		min-height: 0;
		overflow-y: auto;
		padding: var(--space-4) var(--space-6);
	}

	h3,
	legend {
		margin: 0 0 var(--space-2);
		padding: 0;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		font-weight: 600;
		letter-spacing: 0.04em;
		text-transform: uppercase;
	}

	.count {
		margin-left: var(--space-1);
		font-family: var(--font-mono);
		font-weight: 400;
	}

	ul {
		max-height: 11rem;
		margin: 0;
		overflow-y: auto;
		padding: 0;
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		list-style: none;
	}

	li {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		column-gap: var(--space-2);
		padding: var(--space-2) var(--space-3);
		font-size: var(--font-size-sm);
	}

	li + li {
		border-top: var(--border-width) solid var(--border);
	}

	.where {
		font-family: var(--font-mono);
		font-size: var(--font-size-code);
		overflow-wrap: anywhere;
	}

	.side {
		color: var(--muted-fg);
	}

	.line {
		flex-basis: 100%;
		min-width: 0;
		overflow: hidden;
		color: var(--muted-fg);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.summary :global(.cm-editor) {
		min-height: 6lh;
		max-height: 40dvh;
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
	}

	.summary :global(.cm-editor.cm-focused) {
		border-color: var(--status-working);
	}

	.summary :global(.cm-gutters) {
		display: none;
	}

	fieldset {
		display: grid;
		gap: var(--space-1);
		margin: 0;
		padding: 0;
		border: none;
	}

	.choice {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr);
		column-gap: var(--space-3);
		align-items: baseline;
		padding: var(--space-2);
		border-radius: var(--radius-md);
		cursor: pointer;
	}

	.choice:hover {
		background: var(--hover);
	}

	.choice:has(input:checked) {
		background: color-mix(in oklab, var(--status-working) 10%, transparent);
	}

	.choice input {
		grid-row: span 2;
		margin: 0;
		accent-color: var(--status-working);
	}

	.label {
		font-weight: 600;
	}

	.hint,
	.none {
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.none {
		margin: 0;
	}

	.error {
		margin: 0;
		padding: var(--space-2);
		border-left: 3px solid var(--git-deleted);
		background: var(--surface-2);
		overflow-wrap: anywhere;
		white-space: pre-wrap;
	}

	@media (max-width: 40rem) {
		.body {
			padding: var(--space-3) var(--space-4);
		}
	}
</style>
