<script lang="ts" module>
	import type { Agent } from '#lib/config.svelte.ts';
	import type { Variant } from '#lib/ui/TabIcon.svelte';
	import type { Status } from '#lib/ui/TabMarks.svelte';

	export type TabEntry = { id: string; variant: Variant; label: string; detail?: string; agent?: Agent; status?: Status; dirty?: boolean };
</script>

<script lang="ts">
	import type { Git } from '#lib/git.ts';
	import { available, type Section } from '#lib/shell/Sidebar.svelte';
	import { settleInto, swipe } from '#lib/swipe.ts';
	import Button from '#lib/ui/Button.svelte';
	import Checkbox from '#lib/ui/Checkbox.svelte';
	import ListItem from '#lib/ui/ListItem.svelte';
	import ListLabel from '#lib/ui/ListLabel.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import NavList from '#lib/ui/NavList.svelte';
	import TabIcon from '#lib/ui/TabIcon.svelte';
	import TabMarks from '#lib/ui/TabMarks.svelte';
	import type { Snippet } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import Article from '~icons/ph/article';
	import CaretRight from '~icons/ph/caret-right';
	import Files from '~icons/ph/files';
	import GitCommit from '~icons/ph/git-commit';
	import GitPullRequest from '~icons/ph/git-pull-request';
	import Plus from '~icons/ph/plus';

	type Props = {
		tabs: TabEntry[];
		current?: string;
		git?: Git;
		menu(id: string): { label: string; onselect: () => void }[];
		onchoose(id: string): void;
		/** Closes Tabs, returning whether they closed without first asking. */
		onclose(ids: string[]): boolean;
		onsection(id: Section): void;
		onnew(): void;
		empty: Snippet;
	};

	let { tabs, current, git, menu, onchoose, onclose, onsection, onnew, empty }: Props = $props();
	const id = $props.id();
	const icons = { files: Files, git: GitCommit, compare: GitPullRequest, epics: Article };
	const changes = $derived(git?.changes.length);
	let selecting = $state(false);
	let swiping = $state<string>();
	const selected = new SvelteSet<string>();
	const picked = $derived(tabs.filter((tab) => selected.has(tab.id)).map((tab) => tab.id));
	// Selection ends once every picked Tab has closed, including after a confirm.
	const choosing = $derived(selecting && !(selected.size && !picked.length));

	function select(on: boolean) {
		selecting = on;
		selected.clear();
	}

	function choose(event: MouseEvent & { currentTarget: HTMLElement }, tab: string) {
		// A long-press opens the menu on a timer, and its release still clicks.
		if (event.currentTarget.dataset.state !== 'open') onchoose(tab);
	}

	function swipeAway(tab: string) {
		return (row: HTMLElement) => {
			const slide = row.querySelector<HTMLElement>('.slide')!;
			return swipe({
				share: 0.35,
				direction: -1,
				allow: () => !choosing,
				drag: (dx) => {
					swiping = tab;
					slide.style.transform = `translateX(${Math.min(0, dx)}px)`;
				},
				release: (dx, committed) => {
					const closed = committed && onclose([tab]);
					const to = closed ? 'translateX(-100%)' : 'none';
					slide.style.transform = closed ? to : '';
					settleInto(slide, [{ transform: `translateX(${Math.min(0, dx)}px)` }, { transform: to }]);
					if (!closed) swiping = undefined;
				}
			})(row);
		};
	}
</script>

<div class="overview">
	<div class="scroll">
		{#if tabs.length}
			<section aria-labelledby="{id}-tabs">
				<div class="heading">
					<ListLabel element="h3" id="{id}-tabs">Tabs · {tabs.length}</ListLabel>
					<Button variant="link" onclick={() => select(!choosing)}>{choosing ? 'Done' : 'Select'}</Button>
				</div>
				<NavList element="ul" aria-label="Open Tabs">
					{#each tabs as tab, index (tab.id)}
						{#snippet row()}
							<span class="icon"><TabIcon variant={tab.variant} agent={tab.agent} label={tab.label} /></span>
							<span class="text">
								<span class="label" class:mono={tab.variant === 'terminal'} id="{id}-tab-{index}">
									{#if tab.variant === 'terminal'}<bdi>{tab.label}</bdi>{:else}{tab.label}{/if}
								</span>
								{#if tab.detail}<span class="detail">{tab.detail}</span>{/if}
							</span>
							<TabMarks status={tab.status} dirty={tab.dirty} />
						{/snippet}
						<li class="tab" class:swiping={swiping === tab.id} {@attach swipeAway(tab.id)}>
							<span class="reveal" aria-hidden="true">Close</span>
							<div class="slide">
								{#if choosing}
									<ListItem element="label" size="md">
										<Checkbox aria-labelledby="{id}-tab-{index}" bind:checked={() => selected.has(tab.id), (on) => (on ? selected.add(tab.id) : selected.delete(tab.id))} />
										{@render row()}
									</ListItem>
								{:else}
									<Menu context items={menu(tab.id)}>
										{#snippet trigger(props)}
											<ListItem
												{...props}
												element="button"
												size="md"
												data-tab={tab.id}
												aria-current={tab.id === current || undefined}
												aria-labelledby="{id}-tab-{index}"
												onclick={(event: MouseEvent & { currentTarget: HTMLElement }) => choose(event, tab.id)}
											>
												{@render row()}
											</ListItem>
										{/snippet}
									</Menu>
								{/if}
							</div>
						</li>
					{/each}
				</NavList>
			</section>
		{:else}
			<div class="empty">{@render empty()}</div>
		{/if}
		<section aria-labelledby="{id}-browse">
			<ListLabel element="h3" id="{id}-browse">Browse</ListLabel>
			<NavList element="ul">
				{#each available(git) as section (section.id)}
					{@const Icon = icons[section.id]}
					<li>
						<ListItem element="button" size="md" onclick={() => onsection(section.id)}>
							<span class="icon"><Icon /></span>
							<span class="text"><span class="label">{section.label}</span></span>
							{#if section.id === 'git' && changes}<span class="count">{changes} {changes === 1 ? 'change' : 'changes'}</span>{/if}
							<span class="icon"><CaretRight /></span>
						</ListItem>
					</li>
				{/each}
			</NavList>
		</section>
	</div>
	{#if tabs.length}
		<div class="dock">
			{#if choosing}
				<Button variant="danger" disabled={!picked.length} onclick={() => onclose(picked)}>Close {picked.length} {picked.length === 1 ? 'Tab' : 'Tabs'}</Button>
			{:else}
				<Button onclick={onnew}><Plus /> New Tab</Button>
			{/if}
		</div>
	{/if}
</div>

<style>
	.overview {
		display: flex;
		flex: 1;
		flex-direction: column;
		min-height: 0;
	}

	.scroll {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: var(--space-2);
		min-height: 0;
		padding: var(--space-2);
		overflow-y: auto;
		overscroll-behavior-y: contain;
	}

	.empty {
		display: flex;
		flex: none;
		flex-direction: column;
	}

	.heading {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding-right: var(--space-2);
	}

	.tab {
		position: relative;
		overflow: hidden;
	}

	.slide {
		position: relative;
		background: var(--color-bg);
	}

	.reveal {
		position: absolute;
		inset: 0;
		display: none;
		justify-content: flex-end;
		align-items: center;
		padding-right: var(--space-6);
		background: var(--ansi-1);
		color: var(--color-bg);
		font-weight: 600;
	}

	.swiping > .reveal {
		display: flex;
	}

	.icon {
		display: inline-grid;
		flex: none;
		place-items: center;
		width: 1.25em;
		color: var(--muted-fg);
	}

	.text {
		display: flex;
		flex: 1;
		flex-direction: column;
		min-width: 0;
		padding-block: var(--space-1);
	}

	.label,
	.detail {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	/* Right-to-left overflow puts the ellipsis before a Terminal's path, keeping the end that tells Terminals apart. */
	.mono {
		direction: rtl;
		font-family: var(--font-mono);
		text-align: left;
	}

	.detail,
	.count {
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.count {
		flex: none;
	}

	.dock {
		display: flex;
		flex: none;
		flex-direction: column;
		padding: var(--space-2);
		border-top: var(--border-width) solid var(--border);
		background: var(--surface-1);
	}
</style>
