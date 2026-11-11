<script lang="ts">
	import { ContextMenu, DropdownMenu } from 'bits-ui';
	import ListItem from '#lib/ui/ListItem.svelte';
	import ListLabel from '#lib/ui/ListLabel.svelte';
	import Loading from '#lib/ui/Loading.svelte';
	import type { Component, Snippet } from 'svelte';

	type Item = { label: string; onselect: () => void; disabled?: boolean; icon?: Component; hint?: string; checked?: boolean; keepOpen?: boolean; data?: unknown };
	type Group = { heading: string; items: Item[]; empty?: string };
	type Layer = { wrapperProps: Record<string, unknown>; props: Record<string, unknown> };

	type Props = {
		items: (Item | Group)[];
		context?: boolean;
		trigger: Snippet<[Record<string, unknown>]>;
		row?: Snippet<[unknown]>;
		open?: boolean;
		resize?: boolean;
		loading?: string;
		side?: 'top' | 'right' | 'bottom' | 'left';
		align?: 'start' | 'end';
	};

	let { items, context = false, trigger, row: custom, open = $bindable(false), resize = false, loading, side = 'bottom', align = 'start' }: Props = $props();
	let width = $state(0);
</script>

{#snippet row(props: Record<string, unknown>, { label, checked, icon: Icon, hint, data }: Item)}
	<ListItem {...props}>
		{#if custom && data !== undefined}
			{@render custom(data)}
		{:else}
			{#if Icon}<Icon />{/if}
			<span class="label" class:checked>{label}</span>
			{#if hint}<span class="hint" aria-hidden="true">{hint}</span>{/if}
			{#if checked}<span class="check" aria-hidden="true">✓</span>{/if}
		{/if}
	</ListItem>
{/snippet}

{#snippet item(entry: Item)}
	{#if entry.checked === undefined}
		<DropdownMenu.Item onSelect={entry.onselect} disabled={entry.disabled}>
			{#snippet child({ props })}
				{@render row(props, entry)}
			{/snippet}
		</DropdownMenu.Item>
	{:else}
		<DropdownMenu.CheckboxItem checked={entry.checked} onSelect={entry.onselect} disabled={entry.disabled} closeOnSelect={!entry.keepOpen}>
			{#snippet child({ props })}
				{@render row(props, entry)}
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
									<ListLabel {...props}>{entry.heading}</ListLabel>
								{/snippet}
							</DropdownMenu.GroupHeading>
							{#each entry.items as grouped}
								{@render item(grouped)}
							{:else}
								{#if entry.empty}<ListLabel>{entry.empty}</ListLabel>{/if}
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
			<DropdownMenu.Content {side} {align} sideOffset={4} child={content} />
		</DropdownMenu.Portal>
	</DropdownMenu.Root>
{/if}

<style>
	.menu {
		z-index: 50;
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


	.separator {
		margin: var(--space-1) 0;
		border-top: var(--border-width) solid var(--border);
	}

	.checked {
		font-weight: 600;
	}

	.check {
		flex: none;
		padding-left: var(--space-4);
		font-weight: 600;
	}
</style>
