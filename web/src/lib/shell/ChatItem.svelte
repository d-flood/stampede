<script lang="ts">
	import { ago, logos } from '#lib/chat/sessions.ts';
	import type { Agent } from '#lib/config.svelte.ts';
	import ListItem from '#lib/ui/ListItem.svelte';
	import TabMarks, { type Status } from '#lib/ui/TabMarks.svelte';
	import { Command } from 'bits-ui';

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
		/** Wraps the title instead of truncating it. */
		full?: boolean;
		/** Renders a button for lists outside a Command menu. */
		plain?: boolean;
		onselect(): void;
	};

	let { value, agent, title, time, project, status, fresh = false, size, full = false, plain = false, onselect }: Props = $props();

	const Icon = $derived(logos[agent]);
</script>

{#snippet row(props: Record<string, unknown>)}
	<ListItem {...props} {size}>
		<Icon />
		<span class="title" class:full>{title}</span>
		{#if status}<TabMarks {status} {fresh} />{/if}
		<span class="meta">
			{#if project}<span class="project">{project}</span><span aria-hidden="true">·</span>{/if}
			<span aria-hidden="true">{ago(time)}</span>
		</span>
	</ListItem>
{/snippet}

{#if plain}
	{@render row({ element: 'button', onclick: onselect })}
{:else}
	<Command.Item {value} onSelect={onselect}>
		{#snippet child({ props })}
			{@render row(props)}
		{/snippet}
	</Command.Item>
{/if}

<style>
	.title {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.full {
		overflow-wrap: anywhere;
		white-space: normal;
	}

	/* Caps a long Project label so the title keeps most of the row. */
	.meta {
		display: flex;
		flex: none;
		gap: var(--space-1);
		max-width: 50%;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		white-space: nowrap;
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
