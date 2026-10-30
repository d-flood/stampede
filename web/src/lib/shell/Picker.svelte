<script lang="ts">
	import TextField from '#lib/ui/TextField.svelte';
	import { Command, computeCommandScore } from 'bits-ui';

	let { directories, onopen }: { directories: string[]; onopen(path: string): Promise<string | undefined> } =
		$props();

	let query = $state('');
	let error = $state<string>();

	const typed = $derived(/^[~/]/.test(query) ? query : undefined);
	const matches = $derived(
		query
			? directories
					.map((path) => ({ path, score: computeCommandScore(typed ? path : name(path), query) }))
					.filter(({ score }) => score > 0)
					.sort((a, b) => b.score - a.score)
					.map(({ path }) => path)
			: directories
	);

	function name(path: string) {
		return path.slice(path.lastIndexOf('/') + 1);
	}

	async function open(path: string) {
		error = await onopen(path);
	}
</script>

<Command.Root shouldFilter={false} loop>
	<Command.Input
		value={query}
		oninput={() => (error = undefined)}
		placeholder="Find a folder, or type a path"
		aria-invalid={error !== undefined}
		autocomplete="off"
		spellcheck={false}
	>
		{#snippet child({ props })}
			<TextField {...props} mono bind:value={query} {@attach (element) => element.focus()} />
		{/snippet}
	</Command.Input>
	{#if error}
		<p class="error" role="alert">{error}</p>
	{/if}
	<Command.List>
		{#snippet child({ props })}
			<div {...props} class="list">
				<Command.Viewport>
					{#if typed}
						<Command.Item value={`open:${typed}`} onSelect={() => open(typed)}>
							{#snippet child({ props })}
								<div {...props} class="item">Open <span class="path">{typed}</span></div>
							{/snippet}
						</Command.Item>
					{/if}
					{#each matches as path (path)}
						{@const cut = path.lastIndexOf('/') + 1}
						<Command.Item value={path} onSelect={() => open(path)}>
							{#snippet child({ props })}
								<div {...props} class="item">
									<span class="path"><span class="parent">{path.slice(0, cut)}</span>{path.slice(cut)}</span>
								</div>
							{/snippet}
						</Command.Item>
					{/each}
					{#if !typed && matches.length === 0}
						<p class="empty">{directories.length ? 'No matching folder' : 'No folders under the configured roots'}</p>
					{/if}
				</Command.Viewport>
			</div>
		{/snippet}
	</Command.List>
</Command.Root>

<style>
	.error {
		margin: var(--space-1) 0 0;
		color: var(--ansi-1);
		font-size: var(--font-size-sm);
	}

	.list {
		max-height: min(24rem, 50dvh);
		margin-top: var(--space-2);
		overflow-y: auto;
	}

	.item {
		display: flex;
		align-items: center;
		gap: var(--space-1);
		min-height: var(--target-min);
		padding: 0 var(--space-2);
		border-radius: var(--radius-sm);
		cursor: pointer;
		white-space: nowrap;
	}

	.item[data-selected] {
		background: var(--hover);
	}

	.path {
		overflow: hidden;
		font-family: var(--font-mono);
		text-overflow: ellipsis;
	}

	.parent {
		color: var(--muted-fg);
	}

	.empty {
		margin: 0;
		padding: var(--space-2);
		color: var(--muted-fg);
	}
</style>
