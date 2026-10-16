<script lang="ts">
	import { ContextMenu, DropdownMenu } from 'bits-ui';
	import type { Snippet } from 'svelte';

	type Item = { label: string; onselect: () => void; disabled?: boolean };
	type Group = { heading: string; items: Item[] };
	type Layer = { wrapperProps: Record<string, unknown>; props: Record<string, unknown> };

	let {
		items,
		context = false,
		trigger
	}: { items: (Item | Group)[]; context?: boolean; trigger: Snippet<[Record<string, unknown>]> } = $props();
</script>

{#snippet item({ label, onselect, disabled }: Item)}
	<DropdownMenu.Item onSelect={onselect} {disabled}>
		{#snippet child({ props })}
			<div {...props} class="item">{label}</div>
		{/snippet}
	</DropdownMenu.Item>
{/snippet}

{#snippet content({ wrapperProps, props }: Layer)}
	<div {...wrapperProps}>
		<div {...props} class="menu">
			{#each items as entry, index}
				{#if 'heading' in entry}
					<DropdownMenu.Group>
						<DropdownMenu.GroupHeading>
							{#snippet child({ props })}
								<div {...props} class="heading">{entry.heading}</div>
							{/snippet}
						</DropdownMenu.GroupHeading>
						{#each entry.items as grouped (grouped.label)}
							{@render item(grouped)}
						{/each}
					</DropdownMenu.Group>
					{#if index < items.length - 1}
						<DropdownMenu.Separator>
							{#snippet child({ props })}
								<div {...props} class="separator"></div>
							{/snippet}
						</DropdownMenu.Separator>
					{/if}
				{:else}
					{@render item(entry)}
				{/if}
			{/each}
		</div>
	</div>
{/snippet}

{#if context}
	<ContextMenu.Root>
		<ContextMenu.Trigger>
			{#snippet child({ props })}
				{@render trigger(props)}
			{/snippet}
		</ContextMenu.Trigger>
		<ContextMenu.Portal>
			<ContextMenu.Content child={content} />
		</ContextMenu.Portal>
	</ContextMenu.Root>
{:else}
	<DropdownMenu.Root>
		<DropdownMenu.Trigger>
			{#snippet child({ props })}
				{@render trigger(props)}
			{/snippet}
		</DropdownMenu.Trigger>
		<DropdownMenu.Portal>
			<DropdownMenu.Content sideOffset={4} align="start" child={content} />
		</DropdownMenu.Portal>
	</DropdownMenu.Root>
{/if}

<style>
	.menu {
		min-width: 10rem;
		padding: var(--space-1);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-2);
		box-shadow: var(--shadow-float);
		color: var(--color-fg);
		font-family: var(--font-ui);
		font-size: var(--font-size-md);
		outline: none;
	}

	.item {
		display: flex;
		align-items: center;
		min-height: var(--target-min);
		padding: 0 var(--space-2);
		border-radius: var(--radius-sm);
		cursor: pointer;
		outline: none;
	}

	.heading {
		display: flex;
		align-items: center;
		min-height: var(--target-min);
		padding: 0 var(--space-2);
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.separator {
		margin: var(--space-1) 0;
		border-top: var(--border-width) solid var(--border);
	}

	.item[data-highlighted] {
		background: var(--hover);
	}

	.item[data-disabled] {
		color: var(--muted-fg);
		cursor: default;
	}
</style>
