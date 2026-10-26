<script lang="ts">
	import { ContextMenu, DropdownMenu } from 'bits-ui';
	import Loading from '#lib/ui/Loading.svelte';
	import type { Component, Snippet } from 'svelte';

	type Item = { label: string; onselect: () => void; disabled?: boolean; icon?: Component; hint?: string; checked?: boolean; keepOpen?: boolean };
	type Group = { heading: string; items: Item[] };
	type Layer = { wrapperProps: Record<string, unknown>; props: Record<string, unknown> };

	type Props = { items: (Item | Group)[]; context?: boolean; trigger: Snippet<[Record<string, unknown>]>; open?: boolean; resize?: boolean; loading?: string };

	let { items, context = false, trigger, open = $bindable(false), resize = false, loading }: Props = $props();
	let width = $state(0);
</script>

{#snippet item({ label, onselect, disabled, checked, icon: Icon, hint, keepOpen }: Item)}
	{#if checked === undefined}
		<DropdownMenu.Item onSelect={onselect} {disabled}>
			{#snippet child({ props })}
				<div {...props} class="item">
					{#if Icon}<Icon />{/if}
					<span class="label">{label}</span>
					{#if hint}<span class="hint" aria-hidden="true">{hint}</span>{/if}
				</div>
			{/snippet}
		</DropdownMenu.Item>
	{:else}
		<DropdownMenu.CheckboxItem {checked} onSelect={onselect} {disabled} closeOnSelect={!keepOpen}>
			{#snippet child({ props })}
				<div {...props} class="item">
					{#if Icon}<Icon />{/if}
					<span class="label">{label}</span>
					{#if hint}<span class="hint" aria-hidden="true">{hint}</span>{/if}
				</div>
			{/snippet}
		</DropdownMenu.CheckboxItem>
	{/if}
{/snippet}

{#snippet content({ wrapperProps, props }: Layer)}
	<div {...wrapperProps}>
		<div {...props} class="menu" class:resizing={resize} style:width={resize && width ? `calc(${width}px + 2 * var(--border-width))` : undefined}>
			<div class="content" class:measured={resize} bind:offsetWidth={width}>
				{#if loading}
					<div class="loading"><Loading label={loading} /></div>
				{/if}
				{#each items as entry, index}
					{#if index > 0 && ('heading' in entry || 'heading' in items[index - 1])}
						<DropdownMenu.Separator>
							{#snippet child({ props })}
								<div {...props} class="separator"></div>
							{/snippet}
						</DropdownMenu.Separator>
					{/if}
					{#if 'heading' in entry}
						<DropdownMenu.Group>
							<DropdownMenu.GroupHeading>
								{#snippet child({ props })}
									<div {...props} class="heading">{entry.heading}</div>
								{/snippet}
							</DropdownMenu.GroupHeading>
							{#each entry.items as grouped}
								{@render item(grouped)}
							{/each}
						</DropdownMenu.Group>
					{:else}
						{@render item(entry)}
					{/if}
				{/each}
			</div>
		</div>
	</div>
{/snippet}

{#if context}
	<ContextMenu.Root bind:open>
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
	<DropdownMenu.Root bind:open>
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
		box-sizing: border-box;
		min-width: 10rem;
		max-width: min(24rem, 100vw - 2 * var(--space-2));
		max-height: min(var(--bits-floating-available-height), 100dvh - 2 * var(--space-2));
		overflow-y: auto;
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--surface-2);
		box-shadow: var(--shadow-float);
		color: var(--color-fg);
		font-family: var(--font-ui);
		font-size: var(--font-size-md);
		outline: none;
	}

	.resizing {
		overflow-x: hidden;
		transition: width var(--dur-med) ease-out;
	}

	.content {
		box-sizing: border-box;
		padding: var(--space-1);
	}

	.measured {
		width: max-content;
		min-width: calc(10rem - 2 * var(--border-width));
		max-width: calc(min(24rem, 100vw - 2 * var(--space-2)) - 2 * var(--border-width));
	}

	.loading {
		display: flex;
		align-items: center;
		min-height: var(--target-min);
		padding: 0 var(--space-2);
	}

	.item {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-height: var(--target-min);
		padding: 0 var(--space-2);
		border-radius: var(--radius-md);
		cursor: pointer;
		outline: none;
	}

	.item :global(svg) {
		flex: none;
	}

	.label {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.hint {
		flex: none;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
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

	.item[aria-checked='true'] {
		font-weight: 600;
	}

	.item[aria-checked='true']::after {
		content: '✓';
		margin-left: auto;
		padding-left: var(--space-4);
	}

	.item[data-disabled] {
		color: var(--muted-fg);
		cursor: default;
	}
</style>
