<script lang="ts">
	import Command from '#lib/chat/Command.svelte';
	import type { Shell } from '#lib/chat/shells.ts';
	import { form } from '#lib/form.ts';
	import Chip from '#lib/ui/Chip.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import Popover from '#lib/ui/Popover.svelte';
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
	<Chip {...props}><span class="icon" class:running><Terminal /></span><span>{shells.length} {shells.length === 1 ? 'shell' : 'shells'}</span></Chip>
{/snippet}

<section class="tray-shells" aria-label="Shells">
	{#if form() === 'phone'}
		{@render chip({ onclick: () => (open = true) })}
		<Dialog bind:open full title="Shells">
			<section aria-label="Running shells">{@render list()}</section>
		</Dialog>
	{:else}
		<Popover bind:open label="Shells" side="top" width="40rem" trigger={chip}>
			<section aria-label="Running shells">{@render list()}</section>
		</Popover>
	{/if}
</section>

<style>
	.tray-shells { display: flex; flex: none; }
	.icon { display: inline-flex; }
	.running :global(svg) { color: var(--status-working); }
	strong { color: var(--status-working); font-size: var(--font-size-sm); font-weight: 600; }
	.shells { max-height: 40vh; overflow: auto; }
	.shell { min-width: 0; }
	.owner { color: var(--muted-fg); font-size: var(--font-size-sm); overflow-wrap: anywhere; }
</style>
