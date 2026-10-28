<script lang="ts">
	import { beginFontPreview, defaultFonts, fontVariables, fonts, selectFont, type FontRole } from '#lib/config.svelte.ts';
	import { onDestroy, tick, untrack } from 'svelte';
	import Button from '#lib/ui/Button.svelte';
	import { Command } from 'bits-ui';
	import Check from '~icons/ph/check';

	let { role, onselect }: { role: FontRole; onselect(): void } = $props();

	let families = $state<string[]>([]);
	let query = $state('');
	let loading = $state(true);
	let saving = $state(false);
	const previewing = untrack(() => role);
	let candidate = $state(fonts[previewing]);
	let previewed = $state(fonts[previewing]);
	let pending = $state(false);
	let error = $state<string>();
	let request = 0;
	const needle = $derived(query.toLowerCase().trim());
	const matches = $derived(families.filter((family) => family.toLowerCase().includes(needle)));
	const preview = beginFontPreview(previewing);
	onDestroy(() => preview.end());

	async function load() {
		loading = true;
		error = undefined;
		try {
			const response = await fetch('/api/fonts');
			if (!response.ok) throw new Error('Could not load fonts.');
			families = await response.json();
		} catch (failure) {
			error = failure instanceof Error ? failure.message : 'Could not load fonts.';
		} finally {
			loading = false;
		}
	}

	async function choose(family: string | null) {
		if (saving) return;
		const current = ++request;
		candidate = family;
		pending = true;
		error = undefined;
		try {
			if (await preview.preview(family)) previewed = family;
		} catch (failure) {
			error = failure instanceof Error ? failure.message : 'Could not preview font.';
		} finally {
			if (current === request) pending = false;
		}
	}

	async function apply() {
		if (saving || pending || candidate !== previewed) return;
		saving = true;
		error = undefined;
		try {
			await selectFont(role, candidate);
			onselect();
		} catch (failure) {
			error = failure instanceof Error ? failure.message : 'Could not save font.';
		} finally {
			saving = false;
		}
	}

	load();
</script>

<p class="current">Saved: {fonts[role] ?? 'Default'}</p>
<div class="preview" role="status" aria-live="polite">
	<span>{pending ? 'Loading preview…' : 'Preview'}</span>
	<strong style:font-family="var({fontVariables[role]})">{candidate ?? defaultFonts[role]}</strong>
</div>
<Command.Root shouldFilter={false} loop>
	<Command.Input placeholder="Search installed fonts" aria-label="Search fonts" autocomplete="off" spellcheck={false}>
		{#snippet child({ props })}
			<input {...props} class="query" bind:value={query} {@attach (element) => { tick().then(() => element.focus()); }} />
		{/snippet}
	</Command.Input>
	<Command.List aria-label="Installed fonts" aria-busy={loading || saving}>
		{#snippet child({ props })}
			<div {...props} class="list">
				<Command.Viewport>
					{#if 'default'.includes(needle)}
						<Command.Item value="Default" disabled={saving} onSelect={() => choose(null)}>
							{#snippet child({ props })}
								<div {...props} class="item">
									<span>Default <small>{defaultFonts[role]}</small></span>
									{#if fonts[role] === null}<Check aria-label="Current font" />{/if}
								</div>
							{/snippet}
						</Command.Item>
					{/if}
					{#each matches as family (family)}
						<Command.Item value={family} disabled={saving} onSelect={() => choose(family)}>
							{#snippet child({ props })}
								<div {...props} class="item">
									<span>{family}</span>
									{#if family === fonts[role]}<Check aria-label="Current font" />{/if}
								</div>
							{/snippet}
						</Command.Item>
					{:else}
						<p class="status">{loading ? 'Loading fonts…' : 'No matching fonts'}</p>
					{/each}
				</Command.Viewport>
			</div>
		{/snippet}
	</Command.List>
</Command.Root>
<div class="actions">
	<Button onclick={onselect}>Cancel</Button>
	<Button variant="primary" disabled={saving || pending || candidate !== previewed || candidate === fonts[role]} onclick={apply}>Apply font</Button>
</div>
{#if saving}<p class="status" role="status">Applying font…</p>{/if}
{#if error}
	<p class="error" role="alert">{error}</p>
	{#if families.length === 0}<Button onclick={load}>Retry</Button>{/if}
{/if}

<style>
	.preview {
		margin-bottom: var(--space-3);
		text-align: center;
		overflow-wrap: anywhere;
	}

	.preview span,
	small {
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.preview span {
		display: block;
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
