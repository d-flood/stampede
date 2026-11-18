<script lang="ts">
	import type { TabItem, TabItems } from '#lib/shell/new-tab.ts';
	import Dialog from '#lib/ui/Dialog.svelte';
	import ListItem from '#lib/ui/ListItem.svelte';
	import ListLabel from '#lib/ui/ListLabel.svelte';
	import Loading from '#lib/ui/Loading.svelte';
	import NavList from '#lib/ui/NavList.svelte';
	import { untrack } from 'svelte';

	type Props = { open: boolean; items: TabItems; reading: boolean; onread(): void };

	let { open = $bindable(), items, reading, onread }: Props = $props();
	const id = $props.id();
	let started = false;

	function start(item: TabItem) {
		started = true;
		open = false;
		item.onselect();
	}
</script>

{#snippet item(entry: TabItem, index: string, detail?: string)}
	{@const Icon = entry.icon}
	<li>
		<ListItem element="button" size="md" aria-labelledby="{id}-{index}" aria-describedby={detail && `${id}-${index}-detail`} onclick={() => start(entry)}>
			<span class="icon">{#if Icon}<Icon />{/if}</span>
			<span class="label" id="{id}-{index}">{entry.label}</span>
			{#if detail}<span class="detail" id="{id}-{index}-detail">{detail}</span>{/if}
		</ListItem>
	</li>
{/snippet}

<Dialog bind:open full title="New Tab" oncloseautofocus={(event) => started && (event.preventDefault(), (started = false))}>
	<section class="list" {@attach () => untrack(onread)}>
		<NavList element="ul">
			{#each items.rigs as rig, index (rig.label)}
				{@render item(rig, `rig-${index}`, rig.model)}
			{/each}
			{#each items.tools as tool, index (tool.label)}
				{@render item(tool, `tool-${index}`)}
			{/each}
		</NavList>
	</section>
	{#if reading || items.sessions.length}
		<section aria-labelledby="{id}-recent">
			<ListLabel element="h3" id="{id}-recent">Recent sessions</ListLabel>
			{#if reading}<Loading label="Reading sessions…" />{/if}
			<div class="list">
				<NavList element="ul">
					{#each items.sessions as session, index (index)}
						{@render item(session, `session-${index}`, session.hint)}
					{/each}
					{#if items.search}{@render item(items.search, 'search')}{/if}
				</NavList>
			</div>
		</section>
	{/if}
</Dialog>

<style>
	.list {
		margin: 0 0 var(--space-4);
	}

	.icon {
		display: inline-grid;
		flex: none;
		place-items: center;
		width: 1.25em;
		color: var(--muted-fg);
	}

	.label {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.detail {
		flex: none;
		max-width: 40%;
		overflow: hidden;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
