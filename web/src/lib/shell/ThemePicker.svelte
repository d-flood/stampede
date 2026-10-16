<script lang="ts">
	import { selectedTheme, selectTheme } from '#lib/config.svelte.ts';
	import Button from '#lib/ui/Button.svelte';
	import { Command } from 'bits-ui';
	import Check from '~icons/ph/check';

	let { onselect }: { onselect(): void } = $props();

	let names = $state<string[]>([]);
	let query = $state('');
	let loading = $state(true);
	let saving = $state(false);
	let error = $state<string>();
	const matches = $derived(names.filter((name) => name.toLowerCase().includes(query.toLowerCase().trim())));

	async function load() {
		loading = true;
		error = undefined;
		try {
			const response = await fetch('/api/themes');
			if (!response.ok) throw new Error('Could not load themes.');
			names = await response.json();
		} catch (failure) {
			error = failure instanceof Error ? failure.message : 'Could not load themes.';
		} finally {
			loading = false;
		}
	}

	async function choose(name: string) {
		if (saving) return;
		saving = true;
		error = undefined;
		try {
			await selectTheme(name);
			onselect();
		} catch (failure) {
			error = failure instanceof Error ? failure.message : 'Could not save theme.';
		} finally {
			saving = false;
		}
	}

	load();
</script>

<p class="current">Current: {selectedTheme.name ?? 'Default'}</p>
<Command.Root shouldFilter={false} loop>
	<Command.Input placeholder="Search Ghostty themes" aria-label="Search themes" autocomplete="off" spellcheck={false}>
		{#snippet child({ props })}
			<input {...props} class="query" bind:value={query} />
		{/snippet}
	</Command.Input>
	<Command.List aria-label="Ghostty themes" aria-busy={loading || saving}>
		{#snippet child({ props })}
			<div {...props} class="list">
				<Command.Viewport>
					{#each matches as name (name)}
						<Command.Item value={name} disabled={saving} onSelect={() => choose(name)}>
						{#snippet child({ props })}
							<div {...props} class="item">
								<span>{name}</span>
								{#if name === selectedTheme.name}<Check aria-label="Current theme" />{/if}
							</div>
						{/snippet}
						</Command.Item>
					{:else}
						<p class="status">{loading ? 'Loading themes…' : 'No matching themes'}</p>
					{/each}
				</Command.Viewport>
			</div>
		{/snippet}
	</Command.List>
</Command.Root>
{#if saving}<p class="status" role="status">Applying theme…</p>{/if}
{#if error}
	<p class="error" role="alert">{error}</p>
	{#if names.length === 0}<Button onclick={load}>Retry</Button>{/if}
{/if}

<style>
	.current,
	.status {
		margin: 0 0 var(--space-3);
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		overflow-wrap: anywhere;
	}

	.query {
		box-sizing: border-box;
		width: 100%;
		min-height: var(--target-min);
		padding: 0 var(--space-2);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--color-bg);
		color: var(--color-fg);
		font: inherit;
		outline: none;
	}

	.query:focus-visible {
		border-color: var(--status-working);
	}

	.list {
		max-height: min(24rem, 50dvh);
		margin-top: var(--space-2);
		overflow-y: auto;
	}

	.item {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
		min-height: var(--target-min);
		padding: 0 var(--space-2);
		border-radius: var(--radius-sm);
		cursor: pointer;
	}

	.item span {
		overflow-wrap: anywhere;
	}

	.item[data-selected] {
		background: var(--hover);
	}

	.item[data-disabled] {
		opacity: 0.5;
		cursor: wait;
	}

	.error {
		color: var(--ansi-1);
		overflow-wrap: anywhere;
	}
</style>
