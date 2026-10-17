<script lang="ts">
	import type { Placed } from '#lib/chat/agents.ts';
	import SubagentRow from '#lib/chat/SubagentRow.svelte';
	import { DropdownMenu } from 'bits-ui';
	import type { Snippet } from 'svelte';

	type Props = {
		heading: string;
		entries: Placed[];
		open: boolean;
		trigger: Snippet<[Record<string, unknown>]>;
		onselect(path: string[]): void;
	};

	let { heading, entries, open = $bindable(), trigger, onselect }: Props = $props();
</script>

<DropdownMenu.Root bind:open>
	<DropdownMenu.Trigger>
		{#snippet child({ props })}
			{@render trigger(props)}
		{/snippet}
	</DropdownMenu.Trigger>
	<DropdownMenu.Portal>
		<DropdownMenu.Content side="top" align="end" sideOffset={4}>
			{#snippet child({ wrapperProps, props })}
				<div {...wrapperProps}>
					<div {...props} class="menu">
						<DropdownMenu.Group>
							<DropdownMenu.GroupHeading>
								{#snippet child({ props })}
									<div {...props} class="heading">{heading}</div>
								{/snippet}
							</DropdownMenu.GroupHeading>
							{#each entries as { path, subagent } (path.join('/'))}
								<DropdownMenu.Item onSelect={() => onselect(path)}>
									{#snippet child({ props })}
										<div {...props} class="item"><SubagentRow {subagent} /></div>
									{/snippet}
								</DropdownMenu.Item>
							{/each}
						</DropdownMenu.Group>
					</div>
				</div>
			{/snippet}
		</DropdownMenu.Content>
	</DropdownMenu.Portal>
</DropdownMenu.Root>

<style>
	.menu {
		box-sizing: border-box;
		width: min(28rem, 100vw - 2 * var(--space-4));
		max-height: min(var(--bits-floating-available-height), 100dvh - 2 * var(--space-2));
		overflow-y: auto;
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

	.heading {
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
		min-height: var(--target-min);
		padding: var(--space-1) var(--space-2);
		border-radius: var(--radius-sm);
		cursor: pointer;
		outline: none;
	}

	.item[data-highlighted] {
		background: var(--hover);
	}
</style>
