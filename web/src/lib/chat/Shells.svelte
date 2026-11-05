<script lang="ts">
	import Command from '#lib/chat/Command.svelte';
	import type { Shell } from '#lib/chat/shells.ts';
	import { form } from '#lib/form.ts';
	import Dialog from '#lib/ui/Dialog.svelte';
	import { Popover } from 'bits-ui';
	import { cubicOut } from 'svelte/easing';
	import { prefersReducedMotion } from 'svelte/motion';
	import { slide } from 'svelte/transition';
	import Terminal from '~icons/ph/terminal';

	let { shells }: { shells: Shell[] } = $props();

	let open = $state(false);
	const running = $derived(shells.filter(({ item }) => item.status === 'running').length);
	const heading = $derived(running ? `Running shells · ${running}` : 'Shells finished');
	const motion = $derived({ duration: prefersReducedMotion.current ? 0 : 250, easing: cubicOut });
</script>

{#snippet list()}
	<strong>{heading}</strong>
	<div class="shells">
		{#each shells as shell (shell.id)}
			<div class="shell" transition:slide={motion}>
				{#if shell.owner !== 'Main'}<span class="owner">{shell.owner}</span>{/if}
				<Command item={shell.item} />
			</div>
		{/each}
	</div>
{/snippet}

{#snippet chip(props: Record<string, unknown>)}
	<button {...props} class="chip" class:running><Terminal /><span>{shells.length} {shells.length === 1 ? 'shell' : 'shells'}</span></button>
{/snippet}

<section class="tray-shells" aria-label="Shells">
	{#if form() === 'phone'}
		{@render chip({ onclick: () => (open = true) })}
		<Dialog bind:open full title="Shells">
			<section aria-label="Running shells">{@render list()}</section>
		</Dialog>
	{:else}
		<Popover.Root bind:open>
			<Popover.Trigger>
				{#snippet child({ props })}
					{@render chip(props)}
				{/snippet}
			</Popover.Trigger>
			<Popover.Portal>
				<Popover.Content side="top" align="start" sideOffset={4} collisionPadding={8}>
					{#snippet child({ wrapperProps, props })}
						<div {...wrapperProps}>
							<section {...props} class="popover" aria-label="Running shells">
								{@render list()}
							</section>
						</div>
					{/snippet}
				</Popover.Content>
			</Popover.Portal>
		</Popover.Root>
	{/if}
</section>

<style>
	.tray-shells { display: flex; flex: none; }
	.chip { display: inline-flex; align-items: center; gap: var(--space-1); min-height: var(--target-min); padding: 0 var(--space-2); border: var(--border-width) solid var(--border); border-radius: calc(var(--target-min) / 2); background: var(--surface-1); color: var(--color-fg); font: inherit; white-space: nowrap; cursor: pointer; }
	.chip:hover { background: var(--hover); }
	.chip :global(svg) { flex: none; color: var(--muted-fg); }
	.chip.running :global(svg) { color: var(--status-working); }
	.popover { box-sizing: border-box; width: min(40rem, 100vw - 2 * var(--space-4)); padding: var(--space-2) var(--space-3); border: var(--border-width) solid var(--border); border-radius: var(--radius-md); background: var(--surface-2); box-shadow: var(--shadow-float); color: var(--color-fg); font-family: var(--font-ui); font-size: var(--font-size-md); outline: none; }
	strong { color: var(--status-working); font-size: var(--font-size-sm); font-weight: 600; }
	.shells { max-height: 40vh; overflow: auto; }
	.shell { min-width: 0; }
	.owner { color: var(--muted-fg); font-size: var(--font-size-sm); overflow-wrap: anywhere; }
</style>
