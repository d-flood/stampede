<script lang="ts">
	import IconButton from '#lib/ui/IconButton.svelte';
	import { Dialog } from 'bits-ui';
	import type { Snippet } from 'svelte';
	import X from '~icons/ph/x';

	type Props = {
		open?: boolean;
		full?: boolean;
		title: string;
		description?: string;
		children?: Snippet;
		actions?: Snippet;
	};

	let { open = $bindable(false), full = false, title, description, children, actions }: Props = $props();
</script>

<Dialog.Root bind:open>
	<Dialog.Portal>
		<Dialog.Overlay>
			{#snippet child({ props })}
				<div {...props} class="overlay"></div>
			{/snippet}
		</Dialog.Overlay>
		<Dialog.Content>
			{#snippet child({ props })}
				<div {...props} class="dialog" class:full>
					<div class="heading">
						<Dialog.Title>
							{#snippet child({ props })}
								<h2 {...props}>{title}</h2>
							{/snippet}
						</Dialog.Title>
						{#if full}
							<Dialog.Close>
								{#snippet child({ props })}
									<IconButton {...props} label="Close"><X /></IconButton>
								{/snippet}
							</Dialog.Close>
						{/if}
					</div>
					{#if description}
						<Dialog.Description>
							{#snippet child({ props })}
								<p {...props}>{description}</p>
							{/snippet}
						</Dialog.Description>
					{/if}
					{@render children?.()}
					{#if actions}
						<footer>{@render actions()}</footer>
					{/if}
				</div>
			{/snippet}
		</Dialog.Content>
	</Dialog.Portal>
</Dialog.Root>

<style>
	.overlay {
		position: fixed;
		inset: 0;
		z-index: 50;
		background: color-mix(in oklab, var(--color-bg) 60%, transparent);
	}

	.dialog {
		position: fixed;
		top: 50%;
		left: 50%;
		z-index: 50;
		box-sizing: border-box;
		width: min(28rem, calc(100vw - 2 * var(--space-4)));
		padding: var(--space-4);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--surface-1);
		box-shadow: var(--shadow-float);
		color: var(--color-fg);
		font-family: var(--font-ui);
		transform: translate(-50%, -50%);
		outline: none;
	}

	.full {
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		overflow-y: auto;
		border: none;
		border-radius: 0;
		box-shadow: none;
		transform: none;
	}

	.heading {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
		margin: 0 0 var(--space-2);
	}

	h2 {
		margin: 0;
		font-size: var(--font-size-lg);
	}

	p {
		margin: 0 0 var(--space-3);
		color: var(--muted-fg);
	}

	footer {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-2);
		margin-top: var(--space-4);
	}
</style>
