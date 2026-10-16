<script lang="ts">
	import type { Answer, Request } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';
	import Button from '#lib/ui/Button.svelte';

	type Props = { request: Request; agent: string; disabled: boolean; onanswer(answer: Answer): void };

	let { request, agent, disabled, onanswer }: Props = $props();

	const title = $derived.by(() => {
		switch (request.kind) {
			case 'command':
				return `${agent} wants to run a command`;
			case 'edit':
				return `${agent} wants to edit a file`;
			case 'other':
				return request.title;
		}
	});

	function quote(word: string) {
		return /^[\w@%+=:,./-]+$/.test(word) ? word : `'${word.replaceAll("'", `'\\''`)}'`;
	}
</script>

<section aria-label="Approval" class="card" class:phone={form() === 'phone'}>
	<h2>{title}</h2>
	{#if request.kind === 'command'}
		<pre class="code">{request.words.map((words) => words.map(quote).join(' ')).join('\n')}</pre>
	{:else if request.kind === 'edit'}
		<div class="path">{request.path}</div>
		<div class="code">
			{#each request.diff.trimEnd().split('\n') as line, index (index)}
				<div class="line" class:added={line.startsWith('+')} class:removed={line.startsWith('-')}>{line}</div>
			{/each}
		</div>
	{:else}
		<pre class="code">{request.detail}</pre>
	{/if}
	<div class="actions">
		<Button variant="primary" {disabled} onclick={() => onanswer({ allow: true })}>Allow</Button>
		{#if request.kind === 'command'}
			<Button disabled>Always allow</Button>
		{/if}
		<Button variant="danger" {disabled} onclick={() => onanswer({ allow: false })}>Deny</Button>
	</div>
</section>

<style>
	.card {
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
	.line {
		white-space: pre-wrap;
	}

	.added {
		color: var(--git-added);
	}

	.removed {
		color: var(--git-deleted);
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
