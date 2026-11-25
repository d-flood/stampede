<script lang="ts">
	import ListItem from '#lib/ui/ListItem.svelte';
	import ListLabel from '#lib/ui/ListLabel.svelte';
	import TextField from '#lib/ui/TextField.svelte';
	import { Command, computeCommandScore } from 'bits-ui';
	import FolderSimple from '~icons/ph/folder-simple';

	let { directories, onopen, autofocus = true }: { directories: string[]; onopen(path: string): Promise<string | undefined>; autofocus?: boolean } =
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

	const parents = $derived(new Set(matches.map((path) => path.slice(0, path.lastIndexOf('/')))));
	const shared = $derived(parents.size === 1 && matches.length > 1 ? [...parents][0].replace(/^(?:\/home|\/Users)\/[^/]+/, '~') : undefined);

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
			<TextField {...props} bind:value={query} {@attach (element) => { if (autofocus) element.focus(); }} />
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
								<ListItem {...props}>Open <span class="path">{typed}</span></ListItem>
							{/snippet}
						</Command.Item>
					{/if}
					{#if !typed && matches.length}<ListLabel element="p">{#if shared}<span class="shared">Folders in <bdi>{shared}</bdi></span>{:else}Folders{/if}</ListLabel>{/if}
					{#each matches as path (path)}
						{@const cut = path.lastIndexOf('/') + 1}
						<Command.Item value={path} onSelect={() => open(path)}>
							{#snippet child({ props })}
								<ListItem {...props} aria-label={path}>
									<FolderSimple aria-hidden="true" />
									<span class="folder">{#if !shared}<span class="parent"><bdi>{path.slice(0, cut)}</bdi></span>{/if}<span class="name">{path.slice(cut)}</span></span>
								</ListItem>
							{/snippet}
						</Command.Item>
					{/each}
					{#if !typed && matches.length === 0}
						<ListLabel element="p">{directories.length ? 'No matching folder' : 'No folders under the configured roots'}</ListLabel>
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

	.path {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.parent {
		color: var(--muted-fg);
	}

	.shared {
		display: block;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	/* Reverses visually so the folder name leads, while the path stays in reading order. */
	.folder {
		display: flex;
		flex-direction: row-reverse;
		justify-content: flex-end;
		align-items: baseline;
		gap: var(--space-2);
		min-width: 0;
		white-space: nowrap;
	}

	.folder .name {
		flex: none;
		margin-right: auto;
	}

	/* Right-to-left moves the ellipsis to the path's start, keeping the segments that tell folders apart. */
	.folder .parent {
		min-width: 0;
		overflow: hidden;
		direction: rtl;
		font-size: var(--font-size-sm);
		text-align: left;
		text-overflow: ellipsis;
	}

</style>
