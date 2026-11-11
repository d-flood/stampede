<script lang="ts">
	import type { Git } from '#lib/git.ts';
	import { form } from '#lib/form.ts';
	import { openSettings } from '#lib/shell/Settings.svelte';
	import type { TabItems } from '#lib/shell/new-tab.ts';
	import Button from '#lib/ui/Button.svelte';
	import Card from '#lib/ui/Card.svelte';
	import Kbd from '#lib/ui/Kbd.svelte';
	import ListItem from '#lib/ui/ListItem.svelte';
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
				<Card primary={index === 0} aria-labelledby="{id}-rig-{index}" aria-describedby="{id}-model-{index}" onclick={onselect}>
					<span class="rig">
						{#if Icon}<span class="logo"><Icon /></span>{/if}
						<span class="meta">
							<span class="label" id="{id}-rig-{index}">{label}</span>
							<span class="model" id="{id}-model-{index}">{model}</span>
						</span>
						{#if hint && !phone}<Kbd aria-hidden="true">{hint}</Kbd>{/if}
					</span>
				</Card>
			{:else}
				<Card primary aria-labelledby="{id}-create" aria-describedby="{id}-create-help" onclick={() => openSettings('rigs')}>
					<span class="rig">
						<span class="logo"><Plus /></span>
						<span class="meta">
							<span class="label" id="{id}-create">Create a Rig</span>
							<span class="model" id="{id}-create-help">Pick an Agent, model and mode</span>
						</span>
					</span>
				</Card>
			{/each}
		</div>
		<div class="tools">
			{#each items.tools as { label, icon: Icon, hint, onselect } (label)}
				<button class="tool" onclick={onselect}>
					{#if Icon}<Icon />{/if}
					<span class="label">{label}</span>
					{#if hint && !phone}<Kbd aria-hidden="true">{hint}</Kbd>{/if}
				</button>
			{/each}
		</div>
		{#if reading || items.sessions.length}
			<div class="recent">
				<h3>Recent sessions</h3>
				{#if reading}<Loading label="Reading sessions…" />{/if}
				<div class="sessions">
					{#each items.sessions as { label, icon: Icon, hint, onselect }, index (index)}
						<ListItem element="button" size="md" onclick={onselect}>
							{#if Icon}<Icon />{/if}
							<span class="title">{label}</span>
							{#if hint}<span class="ago" aria-hidden="true">{hint}</span>{/if}
						</ListItem>
					{/each}
				</div>
				{#if items.search}<Button variant="link" onclick={items.search.onselect}>{items.search.label}</Button>{/if}
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
		height: 100%;
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


	.rig > :global(kbd) {
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

	.tool:hover {
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

	.sessions {
		margin: 0 calc(-1 * var(--space-3));
	}

	.title {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	button:focus-visible {
		outline: var(--focus-ring);
		outline-offset: var(--focus-offset);
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

</style>
