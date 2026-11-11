<script lang="ts">
	import { createBranch, refs, switchBranch, sync, type Refs } from '#lib/git.ts';
	import IconButton from '#lib/ui/IconButton.svelte';
	import ListItem from '#lib/ui/ListItem.svelte';
	import ListLabel from '#lib/ui/ListLabel.svelte';
	import Loading from '#lib/ui/Loading.svelte';
	import TextField from '#lib/ui/TextField.svelte';
	import { Command, computeCommandScore } from 'bits-ui';
	import { onMount } from 'svelte';
	import ArrowsClockwise from '~icons/ph/arrows-clockwise';
	import Check from '~icons/ph/check';
	import Plus from '~icons/ph/plus';

	type Props = { projectId: string; current?: string; origin: boolean; onswitch?(): void; onpick?(name: string): void };

	let { projectId, current, origin, onswitch, onpick }: Props = $props();

	let listed = $state<Refs>();
	let query = $state('');
	let busy = $state(false);
	let error = $state<string>();

	const groups = $derived([
		{ heading: 'Local branches', names: find(listed?.local ?? []) },
		{ heading: 'Remote branches', names: find(listed?.remote ?? []) }
	]);
	const name = $derived(query.trim());
	const creatable = $derived(!onpick && !!listed && !!name && !listed.local.includes(name) && !listed.remote.includes(name));

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
		if (onpick) return onpick(name);
		run(async () => {
			if (name !== current) await switchBranch(projectId, name);
			onswitch?.();
		});
	}

	function create() {
		run(async () => {
			await createBranch(projectId, name);
			onswitch?.();
		});
	}
</script>

<Command.Root label="Branches" shouldFilter={false} loop>
	<div class="search">
		<Command.Input placeholder={onpick ? 'Find a branch' : 'Find or create a branch'} autocomplete="off" spellcheck={false}>
			{#snippet child({ props })}
				<TextField {...props} mono bind:value={query} {@attach (element) => element.focus()} />
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
										<ListLabel {...props}>{heading}</ListLabel>
									{/snippet}
								</Command.GroupHeading>
								<Command.GroupItems>
									{#each names as name (name)}
										<Command.Item value={`${heading}:${name}`} disabled={busy} onSelect={() => choose(name)}>
											{#snippet child({ props })}
												<ListItem {...props}>
													<span class="name">{name}</span>
													{#if name === current}<Check role="img" aria-label={onpick ? 'Chosen branch' : 'Current branch'} />{/if}
												</ListItem>
											{/snippet}
										</Command.Item>
									{/each}
								</Command.GroupItems>
							</Command.Group>
						{/if}
					{/each}
					{#if creatable}
						<Command.Item value="create" disabled={busy} onSelect={create}>
							{#snippet child({ props })}
								<ListItem {...props}>
									<Plus aria-hidden="true" />
									<span class="create">Create branch</span>
									<span class="name">{name}</span>
								</ListItem>
							{/snippet}
						</Command.Item>
					{:else if listed && groups.every(({ names }) => !names.length)}
						<ListLabel element="p">{query ? 'No matching branch' : 'No branches yet'}</ListLabel>
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


	.create {
		white-space: nowrap;
	}

	.name {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		font-family: var(--font-mono);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

</style>
