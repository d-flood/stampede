<script lang="ts">
	import { beginThemePreview, selectedTheme, selectTheme } from '#lib/config.svelte.ts';
	import { onDestroy } from 'svelte';
	import Button from '#lib/ui/Button.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import { Command } from 'bits-ui';
	import Check from '~icons/ph/check';
	import CaretLeft from '~icons/ph/caret-left';
	import CaretRight from '~icons/ph/caret-right';

	let { onselect }: { onselect(): void } = $props();

	let names = $state<string[]>([]);
	let query = $state('');
	let loading = $state(true);
	let saving = $state(false);
	let candidate = $state(selectedTheme.name);
	let previewed = $state(selectedTheme.name);
	let pending = $state(false);
	let error = $state<string>();
	let request = 0;
	const matches = $derived(names.filter((name) => name.toLowerCase().includes(query.toLowerCase().trim())));
	const preview = beginThemePreview();
	onDestroy(() => preview.end());

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
		const current = ++request;
		candidate = name;
		pending = true;
		error = undefined;
		try {
			if (await preview.preview(name)) previewed = name;
		} catch (failure) {
			error = failure instanceof Error ? failure.message : 'Could not preview theme.';
		} finally {
			if (current === request) pending = false;
		}
	}

	function cycle(direction: number) {
		const index = matches.indexOf(candidate ?? '');
		const next = index < 0 ? (direction > 0 ? 0 : matches.length - 1) : (index + direction + matches.length) % matches.length;
		if (matches[next]) choose(matches[next]);
	}

	async function apply() {
		if (saving || pending || !candidate || candidate !== previewed) return;
		saving = true;
		error = undefined;
		try {
			await selectTheme(candidate);
			onselect();
		} catch (failure) {
			error = failure instanceof Error ? failure.message : 'Could not save theme.';
		} finally {
			saving = false;
		}
	}

	load();
</script>

<p class="current">Saved: {selectedTheme.name ?? 'Default'}</p>
<div class="cycler">
	<IconButton label="Previous theme" disabled={saving || matches.length === 0} onclick={() => cycle(-1)}><CaretLeft /></IconButton>
	<div class="preview" role="status" aria-live="polite">
		<span>{pending ? 'Loading preview…' : 'Preview'}</span>
		<strong>{candidate ?? 'Default'}</strong>
	</div>
	<IconButton label="Next theme" disabled={saving || matches.length === 0} onclick={() => cycle(1)}><CaretRight /></IconButton>
</div>
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
<div class="actions">
	<Button onclick={onselect}>Cancel</Button>
	<Button variant="primary" disabled={saving || pending || !candidate || candidate !== previewed || candidate === selectedTheme.name} onclick={apply}>Apply theme</Button>
</div>
{#if saving}<p class="status" role="status">Applying theme…</p>{/if}
{#if error}
	<p class="error" role="alert">{error}</p>
	{#if names.length === 0}<Button onclick={load}>Retry</Button>{/if}
{/if}

<style>
	.cycler {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		margin-bottom: var(--space-3);
	}

	.preview {
		flex: 1;
		min-width: 0;
		text-align: center;
		overflow-wrap: anywhere;
	}

	.preview span {
		display: block;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-2);
		margin-top: var(--space-3);
	}

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
