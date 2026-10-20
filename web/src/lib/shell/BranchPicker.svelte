<script lang="ts">
	import { refs, switchBranch, sync, type Refs } from '#lib/git.ts';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Loading from '#lib/ui/Loading.svelte';
	import { Command, computeCommandScore } from 'bits-ui';
	import { onMount } from 'svelte';
	import ArrowsClockwise from '~icons/ph/arrows-clockwise';
	import Check from '~icons/ph/check';

	type Props = { projectId: string; current?: string; origin: boolean; onswitch(): void };

	let { projectId, current, origin, onswitch }: Props = $props();

	let listed = $state<Refs>();
	let query = $state('');
	let busy = $state(false);
	let error = $state<string>();

	const groups = $derived([
		{ heading: 'Local branches', names: find(listed?.local ?? []) },
		{ heading: 'Remote branches', names: find(listed?.remote ?? []) }
	]);

	onMount(() => run(load));

	function find(names: string[]) {
		return query ? names.filter((name) => computeCommandScore(name, query) > 0) : names;
	}

	async function load() {
		listed = await refs(projectId);
	}

	async function refresh() {
		await sync(projectId, 'fetch');
		await load();
	}

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

	function choose(name: string) {
		run(async () => {
			if (name !== current) await switchBranch(projectId, name);
			onswitch();
		});
	}
</script>

<Command.Root label="Branches" shouldFilter={false} loop>
	<div class="search">
		<Command.Input placeholder="Find a branch" autocomplete="off" spellcheck={false}>
			{#snippet child({ props })}
				<input {...props} class="query" bind:value={query} {@attach (element) => element.focus()} />
			{/snippet}
		</Command.Input>
		{#if origin}
			<IconButton label="Fetch" title="Fetch remote branches" disabled={busy} onclick={() => run(refresh)}>
				<ArrowsClockwise />
			</IconButton>
		{/if}
	</div>
	{#if error}<p class="error" role="alert">{error}</p>{/if}
	{#if busy}<div class="loading"><Loading label={listed ? 'Working…' : 'Loading branches…'} /></div>{/if}
	<Command.List>
		{#snippet child({ props })}
			<div {...props} class="list">
				<Command.Viewport>
					{#each groups as { heading, names } (heading)}
						{#if names.length}
							<Command.Group>
								<Command.GroupHeading>
									{#snippet child({ props })}
										<div {...props} class="heading">{heading}</div>
									{/snippet}
								</Command.GroupHeading>
								<Command.GroupItems>
									{#each names as name (name)}
										<Command.Item value={`${heading}:${name}`} disabled={busy} onSelect={() => choose(name)}>
											{#snippet child({ props })}
												<div {...props} class="item">
													<span class="name">{name}</span>
													{#if name === current}<Check role="img" aria-label="Current branch" />{/if}
												</div>
											{/snippet}
										</Command.Item>
									{/each}
								</Command.GroupItems>
							</Command.Group>
						{/if}
					{/each}
					{#if listed && groups.every(({ names }) => !names.length)}
						<p class="empty">{query ? 'No matching branch' : 'No branches yet'}</p>
					{/if}
				</Command.Viewport>
			</div>
		{/snippet}
	</Command.List>
</Command.Root>

<style>
	.search {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}

	.query {
		box-sizing: border-box;
		flex: 1;
		min-width: 0;
		min-height: var(--target-min);
		padding: 0 var(--space-2);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--color-bg);
		color: var(--color-fg);
		font: inherit;
		font-family: var(--font-mono);
		outline: none;
	}

	.query:focus-visible {
		border-color: var(--status-working);
	}

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

	.heading {
		display: flex;
		align-items: center;
		min-height: var(--target-min);
		padding: 0 var(--space-2);
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.item {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-height: var(--target-min);
		padding: 0 var(--space-2);
		border-radius: var(--radius-sm);
		cursor: pointer;
		white-space: nowrap;
	}

	.item[data-selected] {
		background: var(--hover);
	}

	.item[data-disabled] {
		cursor: default;
	}

	.item :global(svg) {
		flex: none;
	}

	.name {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		font-family: var(--font-mono);
		text-overflow: ellipsis;
	}

	.empty {
		margin: 0;
		padding: var(--space-2);
		color: var(--muted-fg);
	}
</style>
