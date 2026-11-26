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
		/** Renders a button for lists outside a Command menu. */
		plain?: boolean;
		onselect(): void;
	};

	let { value, agent, title, time, project, status, fresh = false, size, plain = false, onselect }: Props = $props();

	const Icon = $derived(logos[agent]);
</script>

{#snippet row(props: Record<string, unknown>)}
	<ListItem {...props} {size}>
		<span class="icon"><Icon /></span>
		<span class="title" {title}>{title}</span>
		{#if status}<span class="status"><TabMarks {status} {fresh} /></span>{/if}
		<span class="meta" class:aligned={!!status} class:scoped={!!project}>
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
	.icon {
		display: inline-grid;
		flex: none;
		place-items: center;
		width: 1.25em;
		height: 1.25em;
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
