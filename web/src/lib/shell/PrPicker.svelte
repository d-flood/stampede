<script lang="ts">
	import { openReview, prs, type Pr, type PrReview } from '#lib/pr-reviews.ts';
	import Loading from '#lib/ui/Loading.svelte';
	import TextField from '#lib/ui/TextField.svelte';
	import { Command, computeCommandScore } from 'bits-ui';
	import { onMount } from 'svelte';
	import GitPullRequest from '~icons/ph/git-pull-request';

	type Props = { projectId: string; onopen(review: PrReview): void };

	let { projectId, onopen }: Props = $props();

	let listed = $state.raw<Pr[]>();
	let query = $state('');
	let busy = $state(false);
	let error = $state<string>();

	const typed = $derived(query.trim().replace(/^#/, ''));
	const shown = $derived((listed ?? []).filter((pr) => !query || computeCommandScore(`${pr.number} ${pr.title} ${pr.author}`, typed) > 0));
	const number = $derived(/^\d+$/.test(typed) ? Number(typed) : undefined);
	const direct = $derived(number ?? (/^https?:\/\//.test(typed) ? typed : undefined));

	onMount(() =>
		run(async () => {
			const found = await prs(projectId);
			if (!found.available) throw new Error(found.hidden ? 'This Project has no remote on a GitHub repository.' : found.reason);
			listed = found.prs;
		})
	);

	async function run(action: () => Promise<unknown>) {
		if (busy) return;
		busy = true;
		error = undefined;
		try {
			await action();
		} catch (cause) {
			error = cause instanceof Error ? cause.message : String(cause);
		} finally {
			busy = false;
		}
	}

	function choose(pr: number | string) {
		run(async () => onopen(await openReview(projectId, pr)));
	}
</script>

<Command.Root label="Pull requests" shouldFilter={false} loop>
	<Command.Input placeholder="Find, or type a number or URL" autocomplete="off" spellcheck={false}>
		{#snippet child({ props })}
			<TextField {...props} bind:value={query} {@attach (element) => element.focus()} />
		{/snippet}
	</Command.Input>
	{#if error}<p class="error" role="alert">{error}</p>{/if}
	{#if busy}<div class="loading"><Loading label={listed ? 'Fetching the pull request…' : 'Loading pull requests…'} /></div>{/if}
	<Command.List>
		{#snippet child({ props })}
			<div {...props} class="list">
				<Command.Viewport>
					{#if direct !== undefined}
						<Command.Item value="direct" disabled={busy} onSelect={() => choose(direct)}>
							{#snippet child({ props })}
								<div {...props} class="item">
									<GitPullRequest aria-hidden="true" />
									<span class="title">Open {number === undefined ? direct : `#${number}`}</span>
								</div>
							{/snippet}
						</Command.Item>
					{/if}
					{#each shown as pr (pr.number)}
						<Command.Item value={String(pr.number)} disabled={busy} onSelect={() => choose(pr.number)}>
							{#snippet child({ props })}
								<div {...props} class="item pr">
									<span class="number">#{pr.number}</span>
									<span class="title">{pr.title}</span>
									<span class="meta">
										{pr.author}{#if pr.isDraft}<span> · draft</span>{/if}
										{#if pr.requested}<span class="requested">Review requested</span>{/if}
									</span>
								</div>
							{/snippet}
						</Command.Item>
					{/each}
					{#if listed && !shown.length && direct === undefined}
						<p class="empty">{query ? 'No matching pull request' : 'No open pull requests'}</p>
					{/if}
				</Command.Viewport>
			</div>
		{/snippet}
	</Command.List>
</Command.Root>

<style>
	.error {
		margin: var(--space-2) 0 0;
		color: var(--ansi-1);
		font-size: var(--font-size-sm);
		overflow-wrap: anywhere;
		white-space: pre-wrap;
	}

	.loading {
		margin-top: var(--space-2);
	}

	.list {
		max-height: min(24rem, 50dvh);
		margin-top: var(--space-2);
		overflow-y: auto;
	}

	.item {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-height: var(--target-min);
		padding: var(--space-1) var(--space-2);
		border-radius: var(--radius-sm);
		cursor: pointer;
	}

	.item.pr {
		display: grid;
		grid-template-columns: auto 1fr;
		column-gap: var(--space-2);
		row-gap: 0;
	}

	.item[data-selected] {
		background: var(--hover);
	}

	.item[data-disabled] {
		cursor: default;
		opacity: 0.6;
	}

	.item :global(svg) {
		flex: none;
	}

	.number {
		color: var(--muted-fg);
		font-family: var(--font-mono);
		font-size: var(--font-size-code);
	}

	.title {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.meta {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
		grid-column: 2;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.requested {
		padding: 0 var(--space-1);
		border-radius: var(--radius-sm);
		background: color-mix(in oklab, var(--status-waiting) 18%, transparent);
		color: var(--color-fg);
	}

	.empty {
		margin: 0;
		padding: var(--space-2);
		color: var(--muted-fg);
	}
</style>
