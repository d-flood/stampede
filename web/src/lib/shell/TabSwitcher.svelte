<script lang="ts">
	import type { Agent } from '#lib/config.svelte.ts';
	import type { TabItem, TabItems } from '#lib/shell/new-tab.ts';
	import Dialog from '#lib/ui/Dialog.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import ListItem from '#lib/ui/ListItem.svelte';
	import ListLabel from '#lib/ui/ListLabel.svelte';
	import Loading from '#lib/ui/Loading.svelte';
	import NavList from '#lib/ui/NavList.svelte';
	import { marks, tabIcon, type Status, type Variant } from '#lib/ui/Tab.svelte';
	import { untrack } from 'svelte';
	import X from '~icons/ph/x';

	export type SwitcherTab = { id: string; variant: Variant; label: string; agent?: Agent; status?: Status; dirty?: boolean };

	type Props = {
		open: boolean;
		tabs: SwitcherTab[];
		current?: string;
		items: TabItems;
		reading: boolean;
		onread(): void;
		onchoose(id: string): void;
		onclose(id: string): void;
	};

	let { open = $bindable(), tabs, current, items, reading, onread, onchoose, onclose }: Props = $props();
	const id = $props.id();
	let started = false;

	function choose(tab: string) {
		open = false;
		onchoose(tab);
	}

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

<Dialog bind:open full title="Tabs" oncloseautofocus={(event) => started && (event.preventDefault(), (started = false))}>
	<div class="list">
		<NavList element="ul" aria-label="Open Tabs" {@attach () => untrack(onread)}>
			{#each tabs as tab, index (tab.id)}
				{@const Icon = tabIcon(tab.variant, tab.agent)}
				{@const Mark = tab.status && marks[tab.status]}
				<li class="tab {tab.status ?? ''}">
					<ListItem
						element="button"
						size="md"
						data-tab={tab.id}
						aria-current={tab.id === current || undefined}
						aria-labelledby="{id}-tab-{index}"
						aria-describedby={Mark && `${id}-mark-${index}`}
						onclick={() => choose(tab.id)}
					>
						<span class="icon"><Icon /></span>
						<span class="label" class:mono={tab.variant === 'terminal'} id="{id}-tab-{index}">{tab.label}</span>
						{#if Mark}<span class="mark" role="img" aria-label={tab.status} id="{id}-mark-{index}"><Mark /></span>{/if}
						{#if tab.dirty}<span class="dirty" title="Unsaved changes"></span>{/if}
					</ListItem>
					<IconButton label="Close {tab.label}" onclick={() => ((open = false), onclose(tab.id))}><X /></IconButton>
				</li>
			{/each}
		</NavList>
	</div>
	<section aria-labelledby="{id}-new">
		<ListLabel element="h3" id="{id}-new">New Tab</ListLabel>
		<div class="list">
			<NavList element="ul">
				{#each items.rigs as rig, index (rig.label)}
					{@render item(rig, `rig-${index}`, rig.model)}
				{/each}
				{#each items.tools as tool, index (tool.label)}
					{@render item(tool, `tool-${index}`)}
				{/each}
			</NavList>
		</div>
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

	li {
		display: flex;
		align-items: center;
	}

	li > :global(.item) {
		flex: 1;
		width: auto;
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


	.mono {
		font-family: var(--font-mono);
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

	.dirty {
		flex: none;
		width: 0.5em;
		height: 0.5em;
		border-radius: 50%;
		background: currentColor;
	}

	.mark {
		display: inline-flex;
		flex: none;
		color: var(--mark);
	}

	.working {
		--mark: var(--status-working);
	}

	.waiting {
		--mark: var(--status-waiting);
	}

	.background {
		--mark: var(--status-background);
	}

	.done {
		--mark: var(--status-done);
	}
</style>
