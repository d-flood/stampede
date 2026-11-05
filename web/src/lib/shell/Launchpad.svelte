<script lang="ts">
	import type { Git } from '#lib/git.ts';
	import { form } from '#lib/form.ts';
	import { openSettings } from '#lib/shell/Settings.svelte';
	import type { TabItems } from '#lib/shell/new-tab.ts';
	import Loading from '#lib/ui/Loading.svelte';
	import { untrack } from 'svelte';
	import Plus from '~icons/ph/plus';

	type Props = { name: string; path: string; git?: Git; items: TabItems; reading: boolean; onread: () => void };

	let { name, path, git, items, reading, onread }: Props = $props();

	const id = $props.id();
	const phone = $derived(form() === 'phone');
	const changes = $derived(git?.isRepo ? git.changes.length : undefined);
	const where = $derived(
		[path, git?.branch, changes === undefined ? undefined : `${changes} changed file${changes === 1 ? '' : 's'}`].filter(Boolean).join(', ')
	);

	function enter(node: HTMLElement) {
		untrack(onread);
		if (!document.activeElement?.closest('input, textarea, select, [contenteditable], dialog[open]')) node.querySelector<HTMLElement>('.primary')?.focus();
	}

</script>

<section class="launchpad" class:phone aria-labelledby="{id}-name" {@attach enter}>
	<div class="pad">
		<h2 id="{id}-name">{name}</h2>
		<p class="where">{where}</p>
		<div class="rigs">
			{#each items.rigs as { label, icon: Icon, model, hint, onselect }, index (label)}
				<button class="rig" class:primary={index === 0} aria-labelledby="{id}-rig-{index}" aria-describedby="{id}-model-{index}" onclick={onselect}>
					{#if Icon}<span class="logo"><Icon /></span>{/if}
					<span class="meta">
						<span class="label" id="{id}-rig-{index}">{label}</span>
						<span class="model" id="{id}-model-{index}">{model}</span>
					</span>
					{#if hint && !phone}<kbd aria-hidden="true">{hint}</kbd>{/if}
				</button>
			{:else}
				<button class="rig primary" aria-labelledby="{id}-create" aria-describedby="{id}-create-help" onclick={() => openSettings('rigs')}>
					<span class="logo"><Plus /></span>
					<span class="meta">
						<span class="label" id="{id}-create">Create a Rig</span>
						<span class="model" id="{id}-create-help">Pick an Agent, model and mode</span>
					</span>
				</button>
			{/each}
		</div>
		<div class="tools">
			{#each items.tools as { label, icon: Icon, hint, onselect } (label)}
				<button class="tool" onclick={onselect}>
					{#if Icon}<Icon />{/if}
					<span class="label">{label}</span>
					{#if hint && !phone}<kbd aria-hidden="true">{hint}</kbd>{/if}
				</button>
			{/each}
		</div>
		{#if reading || items.sessions.length}
			<div class="recent">
				<h3>Recent sessions</h3>
				{#if reading}<Loading label="Reading sessions…" />{/if}
				{#each items.sessions as { label, icon: Icon, hint, onselect }, index (index)}
					<button class="session" onclick={onselect}>
						{#if Icon}<Icon />{/if}
						<span class="label">{label}</span>
						{#if hint}<span class="ago" aria-hidden="true">{hint}</span>{/if}
					</button>
				{/each}
				{#if items.search}<button class="search" onclick={items.search.onselect}>{items.search.label}</button>{/if}
			</div>
		{/if}
	</div>
</section>

<style>
	.launchpad {
		display: grid;
		grid-row: 1;
		grid-column: 1 / -1;
		grid-template-columns: minmax(0, 1fr);
		place-items: center;
		padding: var(--space-6);
		overflow-y: auto;
	}

	.pad {
		width: min(34rem, 100%);
	}

	h2 {
		margin: 0;
		font: 500 1.875rem/1.15 var(--font-chat);
		overflow-wrap: anywhere;
	}

	.where {
		margin: var(--space-1) 0 var(--space-6);
		color: var(--muted-fg);
		overflow-wrap: anywhere;
	}

	button {
		border: 0;
		background: none;
		color: inherit;
		font: inherit;
		text-align: left;
		cursor: pointer;
	}

	button :global(svg) {
		flex: none;
	}

	.rigs {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(10rem, 1fr));
		gap: var(--space-3);
	}

	.rig {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--space-3);
		padding: var(--space-4);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--surface-1);
	}

	.rig:hover {
		border-color: var(--border-strong);
		background: var(--hover);
	}

	.rig.primary {
		border-color: var(--accent);
		box-shadow: inset 0 0 0 1px var(--accent);
	}

	.logo {
		display: grid;
		place-items: center;
		font-size: 1.375rem;
	}

	.meta {
		display: flex;
		flex-direction: column;
		min-width: 0;
	}

	.rig .label {
		font-weight: 500;
	}

	.model,
	.ago {
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	kbd {
		padding: 1px var(--space-2);
		border: var(--border-width) solid var(--border);
		border-bottom-width: 2px;
		border-radius: var(--radius-sm);
		background: var(--color-bg);
		color: var(--muted-fg);
		font: 0.75rem var(--font-mono);
		white-space: nowrap;
	}

	.rig kbd {
		margin-top: auto;
	}

	.tools {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
		margin-top: var(--space-3);
	}

	.tool {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		min-height: 2.25rem;
		padding: 0 var(--space-4);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
	}

	.tool:hover,
	.session:hover {
		background: var(--hover);
	}

	.recent {
		margin-top: 1.75rem;
	}

	h3 {
		margin: 0 0 var(--space-2);
		color: var(--muted-fg);
		font: 500 var(--font-size-sm) var(--font-ui);
	}

	.session {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		width: calc(100% + 2 * var(--space-2));
		min-height: 2.375rem;
		margin-left: calc(-1 * var(--space-2));
		padding: 0 var(--space-2);
		border-radius: var(--radius-md);
	}

	.session .label {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.search {
		min-height: var(--target-min);
		padding: 0;
		color: var(--accent);
	}

	.search:hover {
		text-decoration: underline;
	}

	button:focus-visible {
		outline: 2px solid var(--accent);
		outline-offset: 2px;
	}

	.phone {
		place-items: start stretch;
		padding: var(--space-6) 1rem;
	}

	.phone h2 {
		font-size: 1.625rem;
	}

	.phone .rigs {
		grid-template-columns: minmax(0, 1fr);
		gap: var(--space-2);
	}

	.phone .rig {
		flex-direction: row;
		align-items: center;
		min-height: 3.5rem;
		padding: 0 var(--space-4);
	}

	.phone .tools {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
	}

	.phone .tool {
		justify-content: center;
		min-height: 48px;
		padding: 0 var(--space-2);
	}

	.phone .session,
	.phone .search {
		min-height: 48px;
	}
</style>
