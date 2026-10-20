<script lang="ts">
	import { beginThemePreview, selectedTheme, selectTheme, themeLabel } from '#lib/config.svelte.ts';
	import { isDark } from '#lib/scheme.svelte.ts';
	import { onDestroy, tick } from 'svelte';
	import Button from '#lib/ui/Button.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import { Command } from 'bits-ui';
	import Check from '~icons/ph/check';
	import CaretLeft from '~icons/ph/caret-left';
	import CaretRight from '~icons/ph/caret-right';
	import Moon from '~icons/ph/moon';
	import Sun from '~icons/ph/sun';

	type Scheme = 'light' | 'dark';

	let { onselect }: { onselect(): void } = $props();

	const schemes = [
		{ scheme: 'light', label: 'Light', Icon: Sun },
		{ scheme: 'dark', label: 'Dark', Icon: Moon }
	] as const;
	let names = $state<string[]>([]);
	let query = $state('');
	let loading = $state(true);
	let saving = $state(false);
	let scheme = $state<Scheme>(isDark() ? 'dark' : 'light');
	const chosen = $state({ light: selectedTheme.light, dark: selectedTheme.dark });
	const previewed = $state({ light: selectedTheme.light, dark: selectedTheme.dark });
	let pending = $state(false);
	let error = $state<string>();
	let request = 0;
	const matches = $derived(names.filter((name) => name.toLowerCase().includes(query.toLowerCase().trim())));
	const candidate = $derived(chosen[scheme] ?? chosen[other(scheme)]);
	const light = $derived(chosen.light ?? chosen.dark);
	const dark = $derived(chosen.dark ?? chosen.light);
	const spec = $derived(light === dark ? light : `light:${light},dark:${dark}`);
	const ready = $derived(!saving && !pending && spec !== null && chosen.light === previewed.light && chosen.dark === previewed.dark);
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

	function other(slot: Scheme): Scheme {
		return slot === 'light' ? 'dark' : 'light';
	}

	function describe(slot: Scheme) {
		return chosen[slot] ?? (chosen[other(slot)] ? `Same as ${other(slot)}` : 'Default');
	}

	async function show(name: string) {
		const current = ++request;
		pending = true;
		error = undefined;
		try {
			return await preview.preview(name);
		} catch (failure) {
			error = failure instanceof Error ? failure.message : 'Could not preview theme.';
			return false;
		} finally {
			if (current === request) pending = false;
		}
	}

	async function choose(name: string) {
		if (saving) return;
		const slot = scheme;
		chosen[slot] = name;
		if (await show(name)) previewed[slot] = name;
	}

	function pick(slot: Scheme) {
		scheme = slot;
		if (candidate) show(candidate);
	}

	function cycle(direction: number) {
		const index = matches.indexOf(chosen[scheme] ?? '');
		const next = index < 0 ? (direction > 0 ? 0 : matches.length - 1) : (index + direction + matches.length) % matches.length;
		if (matches[next]) choose(matches[next]);
	}

	async function apply() {
		if (!ready || !spec) return;
		saving = true;
		error = undefined;
		try {
			await selectTheme(spec);
			onselect();
		} catch (failure) {
			error = failure instanceof Error ? failure.message : 'Could not save theme.';
		} finally {
			saving = false;
		}
	}

	load();
</script>

<p class="current">Saved: {themeLabel()}</p>
<div class="schemes" role="group" aria-label="Color scheme">
	{#each schemes as { scheme: slot, label, Icon } (slot)}
		<button type="button" aria-pressed={scheme === slot} disabled={saving} onclick={() => pick(slot)}>
			<Icon aria-hidden="true" />
			<span>{label}</span>
			<strong>{describe(slot)}</strong>
		</button>
	{/each}
</div>
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
			<input {...props} class="query" bind:value={query} {@attach (element) => { tick().then(() => element.focus()); }} />
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
								{#if name === selectedTheme[scheme]}<Check aria-label="Current theme" />{/if}
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
	<Button variant="primary" disabled={!ready || spec === selectedTheme.name} onclick={apply}>Apply theme</Button>
</div>
{#if saving}<p class="status" role="status">Applying theme…</p>{/if}
{#if error}
	<p class="error" role="alert">{error}</p>
	{#if names.length === 0}<Button onclick={load}>Retry</Button>{/if}
{/if}

<style>
	.schemes {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: var(--space-2);
		margin-bottom: var(--space-3);
	}

	.schemes button {
		display: grid;
		grid-template-columns: auto 1fr;
		align-items: center;
		column-gap: var(--space-2);
		min-width: 0;
		min-height: var(--target-min);
		padding: var(--space-2) var(--space-3);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-2);
		color: var(--muted-fg);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}

	.schemes button:hover:not(:disabled) {
		background: var(--hover);
	}

	.schemes button:focus-visible {
		outline: var(--border-width) solid var(--status-working);
		outline-offset: var(--border-width);
	}

	.schemes button[aria-pressed='true'] {
		border-color: var(--status-working);
		background: var(--surface-3);
		color: var(--color-fg);
	}

	.schemes strong {
		grid-column: 1 / -1;
		overflow: hidden;
		color: var(--color-fg);
		font-weight: 600;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

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
