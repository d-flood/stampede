<script lang="ts">
	import { ContextMenu, DropdownMenu } from 'bits-ui';
	import type { Snippet } from 'svelte';

	type Item = { label: string; onselect: () => void; disabled?: boolean };
	type Layer = { wrapperProps: Record<string, unknown>; props: Record<string, unknown> };

	let {
		items,
		context = false,
		trigger
	}: { items: Item[]; context?: boolean; trigger: Snippet<[Record<string, unknown>]> } = $props();
</script>

{#snippet content({ wrapperProps, props }: Layer)}
	<div {...wrapperProps}>
		<div {...props} class="menu">
			{#each items as item (item.label)}
				<DropdownMenu.Item onSelect={item.onselect} disabled={item.disabled}>
					{#snippet child({ props })}
						<div {...props} class="item">{item.label}</div>
					{/snippet}
				</DropdownMenu.Item>
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

	.item[data-highlighted] {
		background: var(--hover);
	}

	.item[data-disabled] {
		color: var(--muted-fg);
		cursor: default;
	}
</style>
