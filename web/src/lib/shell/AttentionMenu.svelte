<script lang="ts">
	import { tabOf, type Attention } from '#lib/device.svelte.ts';
	import { DropdownMenu } from 'bits-ui';
	import type { Snippet } from 'svelte';

	type Push = { label: string; disabled: boolean; onselect(): void };

	type Props = {
		items: Attention[];
		push?: Push;
		open: boolean;
		trigger: Snippet<[Record<string, unknown>]>;
		entry: Snippet<[Attention]>;
		onselect(item: Attention): void;
	};

	let { items, push, open = $bindable(), trigger, entry, onselect }: Props = $props();
</script>

<DropdownMenu.Root bind:open>
	<DropdownMenu.Trigger>
		{#snippet child({ props })}
			{@render trigger(props)}
		{/snippet}
	</DropdownMenu.Trigger>
	<DropdownMenu.Portal>
		<DropdownMenu.Content side="right" align="end" sideOffset={4}>
			{#snippet child({ wrapperProps, props })}
				<div {...wrapperProps}>
					<div {...props} class="menu">
						<DropdownMenu.Group>
							<DropdownMenu.GroupHeading>
								{#snippet child({ props })}
									<div {...props} class="heading">Needs attention</div>
								{/snippet}
							</DropdownMenu.GroupHeading>
							{#each items as item (tabOf(item))}
								<DropdownMenu.Item onSelect={() => onselect(item)}>
									{#snippet child({ props })}
										<div {...props} class="item">{@render entry(item)}</div>
									{/snippet}
								</DropdownMenu.Item>
							{:else}
								<div class="empty">Nothing needs attention</div>
							{/each}
						</DropdownMenu.Group>
						{#if push}
							<DropdownMenu.Separator>
								{#snippet child({ props })}
									<div {...props} class="separator"></div>
								{/snippet}
							</DropdownMenu.Separator>
							<DropdownMenu.Item disabled={push.disabled} onSelect={push.onselect}>
								{#snippet child({ props })}
									<div {...props} class="item">{push.label}</div>
								{/snippet}
							</DropdownMenu.Item>
						{/if}
					</div>
				</div>
			{/snippet}
		</DropdownMenu.Content>
	</DropdownMenu.Portal>
</DropdownMenu.Root>

<style>
	.menu {
		width: max-content;
		min-width: 14rem;
		max-width: min(24rem, calc(100vw - 2 * var(--space-4)));
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

	.heading,
	.empty {
		display: flex;
		align-items: center;
		min-height: var(--target-min);
		padding: 0 var(--space-2);
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.item {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		min-height: var(--target-min);
		padding: var(--space-1) var(--space-2);
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

	.separator {
		margin: var(--space-1) 0;
		border-top: var(--border-width) solid var(--border);
	}
</style>
