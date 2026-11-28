<script lang="ts">
	import { agentNames, ago, logos } from '#lib/chat/sessions.ts';
	import type { Agent } from '#lib/config.svelte.ts';
	import type { FoundSession } from '#lib/device.svelte.ts';
	import Highlight from '#lib/ui/Highlight.svelte';
	import ListItem from '#lib/ui/ListItem.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import TabMarks, { type Status } from '#lib/ui/TabMarks.svelte';
	import { Command, mergeProps } from 'bits-ui';

	type Props = {
		value: string;
		agent: Agent;
		title: string;
		/** Milliseconds since the epoch. */
		time: number;
		/** Names the Chat's Project in lists that span Projects. */
		project?: string;
		status?: Status;
		fresh?: boolean;
		size?: 'md';
		/** The matching message that found this Session, and how many of its messages matched. */
		excerpt?: NonNullable<FoundSession['excerpt']> & { matches: number };
		ended?: boolean;
		disabled?: boolean;
		/** Renders a button for lists outside a Command menu. */
		plain?: boolean;
		onselect?(): void;
		/** Offers Dismiss in an Ended row's context menu. */
		ondismiss?(): void;
	};

	let { value, agent, title, time, project, status, fresh = false, size, excerpt, ended = false, disabled = false, plain = false, onselect, ondismiss }: Props = $props();

	const Icon = $derived(logos[agent]);
</script>

{#snippet line()}
	<span class="title" {title}>{title}</span>
	{#if status}<span class="status"><TabMarks {status} {fresh} /></span>{/if}
	<span class="meta" class:aligned={!!status} class:scoped={!!project}>
		{#if project}<span class="project">{project}</span><span aria-hidden="true">·</span>{/if}
		<span aria-hidden="true">{ago(time)}</span>
		{#if ended}<span aria-hidden="true">·</span><span>Ended</span>{/if}
	</span>
{/snippet}

{#snippet row(props: Record<string, unknown>)}
	<ListItem {...props} {size}>
		{#if excerpt}
			<span class="found">
				<span class="icon"><Icon /></span>
				<span class="line">{@render line()}</span>
				<span class="excerpt">
					<span class="quote"><span class="who">{excerpt.who === 'user' ? 'You' : agentNames[agent]}</span> <Highlight text={excerpt.text} ranges={excerpt.ranges} /></span>
					{#if excerpt.matches > 1}<span class="more" title="{excerpt.matches - 1} more matching messages">+{excerpt.matches - 1}</span>{/if}
				</span>
			</span>
		{:else}
			<span class="icon"><Icon /></span>
			{@render line()}
		{/if}
	</ListItem>
{/snippet}

{#snippet item(context: Record<string, unknown>)}
	{#if plain}
		{@render row(mergeProps(context, { element: 'button', onclick: onselect, disabled }))}
	{:else}
		<Command.Item {value} {disabled} onSelect={onselect}>
			{#snippet child({ props })}
				{@render row(mergeProps(context, props))}
			{/snippet}
		</Command.Item>
	{/if}
{/snippet}

{#if ended && ondismiss}
	<Menu context items={[{ label: 'Dismiss', onselect: ondismiss }]}>
		{#snippet trigger(props)}
			{@render item(props)}
		{/snippet}
	</Menu>
{:else}
	{@render item({})}
{/if}

<style>
	.icon {
		display: inline-grid;
		flex: none;
		place-items: center;
		width: 1.25em;
		height: 1.25em;
	}

	/* Indents the excerpt under the title by the row's own icon gap. */
	.found {
		display: grid;
		flex: 1;
		grid-template-columns: auto minmax(0, 1fr);
		align-items: center;
		column-gap: inherit;
		row-gap: var(--space-1);
		min-width: 0;
		padding-block: var(--space-1);
	}

	.line {
		display: flex;
		align-items: center;
		min-width: 0;
	}

	.excerpt {
		display: flex;
		grid-column: 2;
		align-items: flex-end;
		gap: var(--space-2);
		min-width: 0;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.quote {
		display: -webkit-box;
		flex: 1;
		min-width: 0;
		overflow: hidden;
		-webkit-box-orient: vertical;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		overflow-wrap: anywhere;
	}

	.who {
		color: var(--color-fg);
		font-weight: 500;
	}

	.more {
		flex: none;
		font-variant-numeric: tabular-nums;
	}

	.title {
		flex: 1;
		min-width: 0;
		margin-inline-end: var(--space-3);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.status {
		display: inline-grid;
		flex: none;
		place-items: center;
		width: 1.25em;
		height: 1.25em;
	}

	/* Caps a long Project label so the title keeps most of the row. */
	.meta {
		display: flex;
		flex: none;
		justify-content: flex-end;
		gap: var(--space-1);
		max-width: 40%;
		color: var(--muted-fg);
		font-size: calc(var(--font-size-sm) - 1px);
		white-space: nowrap;
	}

	/* Gives a found row's meta more room, since its excerpt already says why it matched. */
	.found .meta {
		max-width: 60%;
	}

	.meta.aligned {
		width: 4.75rem;
	}

	.meta.aligned.scoped {
		width: min(8.75rem, 32%);
	}

	.meta > * {
		flex: none;
	}

	.meta > .project {
		flex: 0 1 auto;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
	}
</style>
