<script lang="ts">
	import IconButton from '#lib/ui/IconButton.svelte';
	import { Dialog } from 'bits-ui';
	import type { Snippet } from 'svelte';
	import X from '~icons/ph/x';

	type Props = {
		open?: boolean;
		full?: boolean;
		/** Rises from the bottom edge, sized to its content. */
		sheet?: boolean;
		wide?: boolean;
		/** False when the dialog's own actions already close it. */
		closeButton?: boolean;
		dismissible?: boolean;
		/** Sizes a wide dialog to its content, up to its usual height, without shrinking while open. */
		fit?: boolean;
		/** Visually hides the title and removes outer padding, for search-led palettes. */
		bare?: boolean;
		title: string;
		/** A navigation column down the dialog's full height, titled by `title`, beside content headed by `heading`. */
		nav?: Snippet;
		heading?: string;
		description?: string;
		children?: Snippet;
		actions?: Snippet;
		oncloseautofocus?: (event: Event) => void;
	};

	let { open = $bindable(false), full = false, sheet = false, wide = false, closeButton = true, dismissible = true, fit = false, bare = false, title, nav, heading, description, children, actions, oncloseautofocus }: Props = $props();
	let content = $state<HTMLElement | null>(null);

	// Focusing here, after the focus scope records what to restore on close, keeps a field that focuses itself from becoming that target.
	function autofocus(event: Event) {
		const field = content?.querySelector<HTMLElement>('[data-autofocus]');
		if (!field) return;
		event.preventDefault();
		field.focus();
	}

	function hold(node: HTMLElement) {
		let tallest = 0;
		const observer = new ResizeObserver(() => {
			if (node.offsetHeight <= tallest) return;
			tallest = node.offsetHeight;
			node.style.setProperty('--tallest', `${tallest}px`);
		});
		observer.observe(node);
		return () => observer.disconnect();
	}
</script>

{#snippet titled()}
	<Dialog.Title>
		{#snippet child({ props })}
			<h2 {...props} class:assistive={bare}>{title}</h2>
		{/snippet}
	</Dialog.Title>
{/snippet}

{#snippet close()}
	{#if (full || sheet || wide) && closeButton}
		<Dialog.Close>
			{#snippet child({ props })}
				<IconButton {...props} data-state={undefined} label="Close"><X /></IconButton>
			{/snippet}
		</Dialog.Close>
	{/if}
{/snippet}

{#snippet body()}
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
{/snippet}

<Dialog.Root bind:open>
	<Dialog.Portal>
		<Dialog.Overlay>
			{#snippet child({ props })}
				<div {...props} class="overlay"></div>
			{/snippet}
		</Dialog.Overlay>
	<Dialog.Content
		bind:ref={content}
		onOpenAutoFocus={autofocus}
		onCloseAutoFocus={oncloseautofocus}
		escapeKeydownBehavior={dismissible ? 'close' : 'ignore'}
		interactOutsideBehavior={dismissible ? 'close' : 'ignore'}
	>
			{#snippet child({ props })}
				<div {...props} class="dialog" class:wide class:full class:sheet class:fit class:bare class:navigated={!!nav} class:footed={!!actions} {@attach wide && fit && !full && hold}>
					{#if nav}
						<div class="nav">
							{@render titled()}
							{@render nav()}
						</div>
						<div class="main">
							<div class="heading">
								<h3>{heading}</h3>
								{@render close()}
							</div>
							{@render body()}
						</div>
					{:else if bare}
						{@render titled()}
						{@render body()}
					{:else}
						<div class="heading">
							{@render titled()}
							{@render close()}
						</div>
						{@render body()}
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
		background: color-mix(in oklab, var(--ansi-0) 45%, transparent);
		backdrop-filter: blur(2px);
		animation: fade var(--dur-med) var(--ease-out);
	}

	.dialog {
		position: fixed;
		top: 50%;
		left: 50%;
		z-index: 50;
		box-sizing: border-box;
		width: min(28rem, calc(100vw - 2 * var(--space-4)));
		max-height: calc(100dvh - 2 * var(--space-4));
		overflow-y: auto;
		padding: var(--space-4);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-xl);
		background: var(--surface-1);
		box-shadow: var(--shadow-float);
		color: var(--color-fg);
		font-family: var(--font-ui);
		transform: translate(-50%, -50%);
		outline: none;
		animation: rise var(--dur-med) var(--ease-out);
	}

	.wide {
		display: flex;
		flex-direction: column;
		width: min(54rem, calc(100vw - 2 * var(--space-4)));
		height: min(42rem, calc(100dvh - 2 * var(--space-4)));
		padding: 0;
		overflow: hidden;
		background: var(--plane);
	}

	.bare {
		display: flex;
		flex-direction: column;
		width: min(32rem, calc(100vw - 2 * var(--space-4)));
		padding: 0;
		overflow: hidden;
		border-color: color-mix(in lab, var(--float) 88%, var(--color-fg));
		--hover: color-mix(in lab, var(--float) 90%, var(--color-fg));
		--bare-radius: calc(var(--radius-xl) - var(--border-width));
		background: var(--float);
	}

	.assistive {
		position: absolute;
		width: 1px;
		height: 1px;
		overflow: hidden;
		clip-path: inset(50%);
	}

	.wide.fit:not(.full) {
		height: auto;
		min-height: min(var(--tallest, 0px), calc(100dvh - 2 * var(--space-4)));
		max-height: min(42rem, calc(100dvh - 2 * var(--space-4)));
	}

	.wide .heading {
		margin: 0;
		padding: var(--space-2) var(--space-2) var(--space-2) var(--space-6);
		border-bottom: var(--border-width) solid var(--border);
	}

	.navigated {
		flex-direction: row;
	}

	.nav {
		display: flex;
		flex: none;
		flex-direction: column;
		box-sizing: border-box;
		width: 12rem;
		padding: 0 var(--space-2) var(--space-3);
		overflow: auto;
		border-right: var(--border-width) solid var(--border);
		background: var(--canvas);
		color: var(--muted-fg);
	}

	.nav h2,
	.main .heading {
		display: flex;
		align-items: center;
		box-sizing: border-box;
		min-height: calc(var(--target-min) + 2 * var(--space-4));
	}

	.nav h2 {
		padding: 0 var(--space-3);
		font-size: var(--font-size-sm);
		font-weight: 600;
	}

	.main {
		display: flex;
		flex: 1;
		flex-direction: column;
		min-width: 0;
	}

	.main .heading {
		margin: 0;
		padding: 0 var(--space-4) 0 var(--space-6);
		border: none;
	}

	h3 {
		margin: 0;
		font-size: var(--font-size-xl);
	}

	.wide footer {
		margin: 0;
		padding: var(--space-3) var(--space-4);
		border-top: var(--border-width) solid var(--border);
	}

	.full {
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		max-height: none;
		overflow-y: auto;
		border: none;
		border-radius: 0;
		box-shadow: none;
		transform: none;
		animation: none;
	}

	.wide.full {
		overflow: hidden;
	}

	.sheet {
		top: auto;
		bottom: 0;
		left: 0;
		width: 100%;
		max-height: 85dvh;
		padding-bottom: max(var(--space-4), env(safe-area-inset-bottom));
		border-width: var(--border-width) 0 0;
		border-radius: var(--radius-xl) var(--radius-xl) 0 0;
		transform: none;
		animation: slide var(--dur-med) var(--ease-out);
	}

	.sheet::before {
		content: '';
		display: block;
		width: calc(2 * var(--tile));
		height: var(--space-1);
		margin: calc(-1 * var(--space-2)) auto var(--space-3);
		border-radius: var(--radius-pill);
		background: var(--border-strong);
	}

	@keyframes slide {
		from {
			translate: 0 100%;
		}
	}

	.full.footed {
		display: flex;
		flex-direction: column;
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

	@keyframes fade {
		from {
			opacity: 0;
		}
	}

	@keyframes rise {
		from {
			opacity: 0;
			scale: 0.98;
		}
	}
</style>
