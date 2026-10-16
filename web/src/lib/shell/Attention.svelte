<script lang="ts">
	import type { Attention } from '#lib/device.ts';
	import { form } from '#lib/form.ts';
	import { enable, notifications, type Notifications } from '#lib/push.ts';
	import Badge from '#lib/ui/Badge.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import { DropdownMenu } from 'bits-ui';
	import Bell from '~icons/ph/bell';

	type Props = { items: Attention[]; projects: Map<string, string>; onselect(item: Attention): void };

	let { items, projects, onselect }: Props = $props();

	const labels = { off: 'Enable notifications', on: 'Notifications on', blocked: 'Notifications blocked' };

	let push = $state<Notifications>('unsupported');
	let sheet = $state(false);
	notifications().then((state) => (push = state));

	async function notify() {
		push = await enable();
	}
</script>

{#snippet trigger(props: Record<string, unknown>)}
	<span class="trigger">
		<IconButton {...props} label="Attention list"><Bell /></IconButton>
		{#if items.length}<span class="count"><Badge tone="waiting">{items.length}</Badge></span>{/if}
	</span>
{/snippet}

{#snippet entry(item: Attention)}
	<span class="label">
		<strong>{item.label}</strong>
		<span class="project">in {projects.get(item.projectId)}</span>
	</span>
	<Badge tone={item.status}>{item.status}</Badge>
{/snippet}

{#if form() === 'phone'}
	{@render trigger({ onclick: () => (sheet = true) })}
	<Dialog bind:open={sheet} full title="Needs attention">
		<ul class="sheet">
			{#each items as item (item.terminalId)}
				<li>
					<button class="item" onclick={() => ((sheet = false), onselect(item))}>{@render entry(item)}</button>
				</li>
			{:else}
				<li class="empty">Nothing needs attention</li>
			{/each}
		</ul>
		{#if push !== 'unsupported'}
			<Button disabled={push !== 'off'} onclick={notify}>{labels[push]}</Button>
		{/if}
	</Dialog>
{:else}
	<DropdownMenu.Root>
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
								{#each items as item (item.terminalId)}
									<DropdownMenu.Item onSelect={() => onselect(item)}>
										{#snippet child({ props })}
											<div {...props} class="item">{@render entry(item)}</div>
										{/snippet}
									</DropdownMenu.Item>
								{:else}
									<div class="empty">Nothing needs attention</div>
								{/each}
							</DropdownMenu.Group>
							{#if push !== 'unsupported'}
								{@const label = labels[push]}
								<DropdownMenu.Separator>
									{#snippet child({ props })}
										<div {...props} class="separator"></div>
									{/snippet}
								</DropdownMenu.Separator>
								<DropdownMenu.Item disabled={push !== 'off'} onSelect={notify}>
									{#snippet child({ props })}
										<div {...props} class="item">{label}</div>
									{/snippet}
								</DropdownMenu.Item>
							{/if}
						</div>
					</div>
				{/snippet}
			</DropdownMenu.Content>
		</DropdownMenu.Portal>
	</DropdownMenu.Root>
{/if}

<style>
	.trigger {
		position: relative;
		display: inline-grid;
	}

	.count {
		position: absolute;
		top: calc(-1 * var(--space-1));
		right: calc(-1 * var(--space-1));
		pointer-events: none;
	}

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

	.sheet {
		margin: 0 0 var(--space-4);
		padding: 0;
		list-style: none;
	}

	.sheet .item {
		box-sizing: border-box;
		width: 100%;
		border: none;
		background: none;
		color: inherit;
		font: inherit;
		text-align: start;
	}

	.item[data-highlighted],
	.sheet .item:hover {
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

	.label {
		flex: 1;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.project {
		color: var(--muted-fg);
	}
</style>
