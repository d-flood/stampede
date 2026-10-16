<script lang="ts">
	import type { Attention } from '#lib/device.ts';
	import { enable, notifications, type Notifications } from '#lib/push.ts';
	import Badge from '#lib/ui/Badge.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import { DropdownMenu } from 'bits-ui';
	import Bell from '~icons/ph/bell';

	type Props = { items: Attention[]; projects: Map<string, string>; onselect(item: Attention): void };

	let { items, projects, onselect }: Props = $props();

	const labels = { off: 'Enable notifications', on: 'Notifications on', blocked: 'Notifications blocked' };

	let push = $state<Notifications>('unsupported');
	notifications().then((state) => (push = state));

	async function notify() {
		push = await enable();
	}
</script>

<DropdownMenu.Root>
	<DropdownMenu.Trigger>
		{#snippet child({ props })}
			<span class="trigger">
				<IconButton {...props} label="Attention list"><Bell /></IconButton>
				{#if items.length}<span class="count"><Badge tone="waiting">{items.length}</Badge></span>{/if}
			</span>
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
										<div {...props} class="item">
											<span class="label">
												<strong>{item.label}</strong>
												<span class="project">in {projects.get(item.projectId)}</span>
											</span>
											<Badge tone={item.status}>{item.status}</Badge>
										</div>
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
