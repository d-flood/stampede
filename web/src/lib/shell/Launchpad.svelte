<script lang="ts">
	import type { Git } from '#lib/git.ts';
	import { form } from '#lib/form.ts';
	import { openSettings } from '#lib/shell/Settings.svelte';
	import type { TabItems } from '#lib/shell/new-tab.ts';
	import Composer from '#lib/shell/Composer.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Card from '#lib/ui/Card.svelte';
	import Chip from '#lib/ui/Chip.svelte';
	import Shortcut from '#lib/ui/Shortcut.svelte';
	import ListItem from '#lib/ui/ListItem.svelte';
	import Loading from '#lib/ui/Loading.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import { untrack } from 'svelte';
	import CaretDown from '~icons/ph/caret-down';
	import GitBranch from '~icons/ph/git-branch';
	import PaperPlaneRight from '~icons/ph/paper-plane-right';
	import Plus from '~icons/ph/plus';

	type Props = { name: string; path: string; git?: Git; items: TabItems; reading: boolean; onread: () => void };

	let { name, path, git, items, reading, onread }: Props = $props();

	const id = $props.id();
	const phone = $derived(form() === 'phone');
	const changes = $derived(git?.isRepo ? git.changes.length : undefined);
	const meta = $derived([git?.branch, changes === undefined ? undefined : changes ? `${changes} changed file${changes === 1 ? '' : 's'}` : 'clean'].filter(Boolean).join(' · '));

	let chosen = $state(0);
	const rig = $derived(items.rigs[Math.min(chosen, items.rigs.length - 1)]);
	const rigMenu = $derived([{ heading: 'Rigs', items: items.rigs.map((entry, index) => ({ label: entry.label, icon: entry.icon, shortcut: entry.shortcut, checked: entry === rig, onselect: () => (chosen = index) })) }]);

	function enter(node: HTMLElement) {
		untrack(onread);
		if (!phone && !document.activeElement?.closest('input, textarea, select, [contenteditable], dialog[open]')) node.querySelector<HTMLElement>('textarea, .primary')?.focus();
	}

	function start(text: string) {
		if (!rig || !text.trim()) return false;
		rig.onprompt(text);
		return true;
	}

</script>

<section class="launchpad" class:phone aria-labelledby="{id}-name" {@attach enter}>
	<div class="pad">
		<h2 id="{id}-name">{name}</h2>
		<p class="where" title={path}>{#if phone}<span class="path"><bdi>{path.replace(/^(?:\/home|\/Users)\/[^/]+/, '~')}</bdi></span>{/if}{#if meta}<span class="meta">{#if git?.branch}<GitBranch aria-hidden="true" />{/if}{meta}</span>{/if}</p>
		{#if rig}
			{@const Logo = rig.icon}
			<Composer id="launchpad:{path}" placeholder="Message {rig.label}" onsend={start}>
				{#snippet actions(text, clear)}
					<Menu side="top" align="start" items={rigMenu}>
						{#snippet trigger(props)}
							<Chip {...props} variant="ghost" aria-label="Rig: {rig.label}">
								{#if Logo}<Logo aria-hidden="true" />{/if}<span>{rig.label}</span><CaretDown aria-hidden="true" />
							</Chip>
						{/snippet}
					</Menu>
					<span class="summary">{rig.summary}</span>
					<span class="fill"></span>
					<Button variant="primary" round aria-label="Send" disabled={!text.trim()} onmousedown={(event) => event.preventDefault()} onclick={() => start(text) && clear()}>
						<PaperPlaneRight />
					</Button>
				{/snippet}
			</Composer>
		{:else}
			<div class="rigs">
				<Card primary aria-labelledby="{id}-create" aria-describedby="{id}-create-help" onclick={() => openSettings('rigs')}>
					<span class="rig">
						<span class="logo"><Plus /></span>
						<span class="meta">
							<span class="label" id="{id}-create">Create a Rig</span>
							<span class="model" id="{id}-create-help">Pick an Agent, model and mode</span>
						</span>
					</span>
				</Card>
			</div>
		{/if}
		<div class="tools">
			{#each items.tools as { label, icon: Icon, shortcut, onselect } (label)}
				<button class="tool" onclick={onselect}>
					{#if Icon}<Icon />{/if}
					<span class="label">{label}</span>
					{#if shortcut && !phone}<span class="hint" aria-hidden="true"><Shortcut keys={shortcut} /></span>{/if}
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

	.launchpad:not(.phone) {
		padding-bottom: calc(var(--space-6) + 16vh);
	}

	.launchpad:not(.phone) h2 {
		text-align: center;
	}

	.launchpad:not(.phone) :is(.where, .tools) {
		align-items: center;
		justify-content: center;
	}

	.launchpad:not(.phone) .path {
		max-width: 100%;
		text-align: center;
	}

	.pad {
		width: min(var(--measure), 100%);
	}

	h2 {
		margin: 0;
		font: 600 calc(2 * var(--font-size-base))/1.2 var(--font-ui);
		overflow-wrap: anywhere;
	}

	.where {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		margin: var(--space-1) 0 var(--space-6);
		color: var(--muted-fg);
		white-space: nowrap;
	}

	/* Right-to-left moves the ellipsis to the path's start, keeping its meaningful tail. */
	.path {
		min-width: 0;
		overflow: hidden;
		direction: rtl;
		text-align: left;
		text-overflow: ellipsis;
	}

	.where .meta {
		display: inline-flex;
		flex: none;
		flex-direction: row;
		align-items: center;
		gap: var(--space-1);
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
		gap: var(--space-2);
	}

	.rig {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		height: 100%;
	}

	.logo {
		display: grid;
		place-items: center;
		font-size: var(--font-size-lg);
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


	.hint {
		margin-left: auto;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		white-space: nowrap;
	}

	.tools {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1);
		margin-top: var(--space-3);
		font-size: var(--font-size-sm);
	}

	.fill {
		flex: 1;
	}

	.summary {
		min-width: 0;
		overflow: hidden;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.tool {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-width: 0;
		padding: var(--space-1) var(--space-3);
		border: var(--border-width) solid transparent;
		border-radius: var(--radius-md);
		color: var(--muted-fg);
	}


	.tool:hover {
		background: var(--hover);
		color: var(--color-fg);
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
