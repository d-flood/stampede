<script lang="ts" module>
	import { selectedTheme, selectTheme, type Palette } from '#lib/config.svelte.ts';
	import { form } from '#lib/form.ts';
	import { curated, type Slot } from '#lib/themes.svelte.ts';

	const palettes = $state<Record<string, Palette>>({});
	const requested = new Set<string>();

	function load(name: string) {
		if (requested.has(name)) return;
		requested.add(name);
		fetch(`/api/themes/${encodeURIComponent(name)}`)
			.then((response) => (response.ok ? response.json() : Promise.reject()))
			.then((theme) => (palettes[name] = theme.dark))
			.catch(() => requested.delete(name));
	}
</script>

<script lang="ts">
	let { slot, labelledby }: { slot: Slot; labelledby: string } = $props();

	let error = $state<string>();

	[...curated.light, ...curated.dark].forEach(load);

	async function choose(name: string) {
		const light = slot === 'light' ? name : (selectedTheme.light ?? curated.light[1]);
		const dark = slot === 'dark' ? name : (selectedTheme.dark ?? curated.dark[2]);
		try {
			await selectTheme(`light:${light},dark:${dark}`);
			error = undefined;
		} catch (failure) {
			error = failure instanceof Error ? failure.message : 'Could not save theme.';
		}
	}
</script>

<div class="cards" class:phone={form() === 'phone'} role="group" aria-labelledby={labelledby}>
	{#each curated[slot] as name (name)}
		{@const palette = palettes[name]}
		<button
			type="button"
			class="card"
			aria-pressed={selectedTheme[slot] === name}
			style:--card-bg={palette?.background}
			style:--card-fg={palette?.foreground}
			style:--card-accent={palette?.palette[4]}
			onclick={() => choose(name)}
		>
			<span class="mini" aria-hidden="true">
				<i class="bubble"></i>
				<i class="prose"></i>
				<i class="prose short"></i>
				<i class="accent"></i>
			</span>
			<span class="name">{name}</span>
		</button>
	{/each}
</div>
{#if error}<p class="error" role="alert">{error}</p>{/if}

<style>
	.cards {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: var(--space-3);
		width: 100%;
	}

	.card {
		--card-surface: color-mix(in oklab, var(--card-bg) 96%, var(--card-fg));
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		min-width: 0;
		padding: var(--space-2);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--card-bg, var(--surface-1));
		color: var(--card-fg, var(--color-fg));
		font: inherit;
		text-align: start;
		cursor: pointer;
		transition: border-color var(--dur-fast);
	}

	.card:hover {
		border-color: var(--border-strong);
	}

	.card[aria-pressed='true'] {
		border-color: var(--accent);
		box-shadow: 0 0 0 1px var(--accent);
	}

	.card:focus-visible {
		outline: var(--focus-ring);
		outline-offset: var(--focus-offset);
	}

	.mini {
		display: flex;
		flex-direction: column;
		gap: 5px;
		height: 78px;
		box-sizing: border-box;
		padding: 9px;
		border-radius: var(--radius-md);
		background: var(--card-surface);
	}

	.mini i {
		display: block;
		height: 7px;
		border-radius: 4px;
		background: color-mix(in lab, var(--card-fg) 30%, var(--card-surface));
	}

	.mini .bubble {
		align-self: flex-end;
		width: 46%;
		height: 14px;
		border-radius: 6px 6px 2px 6px;
		background: color-mix(in oklab, var(--card-bg) 88%, var(--card-fg));
	}

	.mini .prose {
		width: 88%;
	}

	.mini .short {
		width: 64%;
	}

	.mini .accent {
		width: 26%;
		margin-top: auto;
		background: var(--card-accent);
	}

	.name {
		padding: 0 var(--space-1);
		font-weight: 600;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.error {
		margin: var(--space-2) 0 0;
		color: var(--ansi-1);
		font-size: var(--font-size-sm);
	}

	.phone {
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: var(--space-2);
	}
</style>
