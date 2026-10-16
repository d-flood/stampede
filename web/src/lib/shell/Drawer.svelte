<script lang="ts">
	import type { Device, ServerMessage, Terminal } from '#lib/device.ts';
	import { form } from '#lib/form.ts';
	import TerminalView from '#lib/shell/TerminalView.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import Tab from '#lib/ui/Tab.svelte';
	import { onMount, tick } from 'svelte';
	import Plus from '~icons/ph/plus';
	import X from '~icons/ph/x';

	let { device }: { device: Device } = $props();

	let panel: HTMLElement;
	let open = $state(false);
	let terminals = $state<Terminal[]>([]);
	let chosen = $state<string>();
	const views: Record<string, TerminalView> = {};
	let returnTo: HTMLElement | undefined;

	const active = $derived(terminals.find((terminal) => terminal.id === chosen) ?? terminals.at(-1));

	onMount(() => device.listen({ message }));

	function message(message: ServerMessage) {
		switch (message.type) {
			case 'hello':
				terminals = message.terminals.filter((terminal) => terminal.owner === 'drawer');
				break;
			case 'terminals.changed':
				if (message.drawer) terminals = message.terminals;
				break;
			case 'terminal.created':
				if (message.terminal.owner === 'drawer') chosen = message.terminal.id;
				break;
			case 'terminal.title':
				for (const terminal of terminals) {
					if (terminal.id === message.terminalId) terminal.label = message.label;
				}
				break;
		}
	}

	export async function toggle() {
		if (open) {
			if (panel.contains(document.activeElement)) returnTo?.focus();
			open = false;
			return;
		}
		returnTo = document.activeElement instanceof HTMLElement ? document.activeElement : undefined;
		open = true;
		await tick();
		if (active) views[active.id]?.focus();
		else panel.focus();
	}

	function keydown(event: KeyboardEvent) {
		if (event.code !== 'Backquote' || !event.ctrlKey || event.altKey || event.shiftKey || event.metaKey) return;
		event.preventDefault();
		event.stopPropagation();
		toggle();
	}

	function closeTab(terminalId: string) {
		if (terminalId === active?.id) {
			const index = terminals.findIndex((terminal) => terminal.id === terminalId);
			const neighbor = terminals[index + 1] ?? terminals[index - 1];
			if (neighbor) chosen = neighbor.id;
		}
		device.send({ type: 'terminal.close', terminalId });
	}
</script>

<svelte:window onkeydowncapture={keydown} />

<section aria-label="Drawer" tabindex="-1" class:open class:phone={form() === 'phone'} inert={!open} bind:this={panel}>
	<div class="strip">
		<div role="tablist" aria-label="Drawer Tabs">
			{#each terminals as tab (tab.id)}
				<Menu context items={[{ label: 'Close', onselect: () => closeTab(tab.id) }]}>
					{#snippet trigger(props)}
						<Tab
							{...props}
							variant="terminal"
							label={tab.label}
							active={tab.id === active?.id}
							onclick={() => (chosen = tab.id)}
						/>
					{/snippet}
				</Menu>
			{/each}
		</div>
		<Menu items={[{ label: 'Terminal', onselect: () => device.send({ type: 'terminal.create', owner: 'drawer' }) }]}>
			{#snippet trigger(props)}
				<IconButton {...props} label="New Drawer Tab"><Plus /></IconButton>
			{/snippet}
		</Menu>
		{#if form() === 'phone'}
			<IconButton label="Close Drawer" onclick={toggle}><X /></IconButton>
		{/if}
	</div>
	<div class="tabs">
		{#each terminals as terminal (terminal.id)}
			<TerminalView
				bind:this={views[terminal.id]}
				{device}
				terminalId={terminal.id}
				active={terminal.id === active?.id}
			/>
		{/each}
	</div>
</section>

<style>
	section {
		position: fixed;
		top: 0;
		right: 0;
		left: 0;
		display: flex;
		flex-direction: column;
		height: 50dvh;
		border-bottom: var(--border-width) solid var(--border);
		background: var(--color-bg);
		outline: none;
		transform: translateY(-100%);
		transition:
			transform var(--dur-med) ease-out,
			box-shadow var(--dur-med);
	}

	.open {
		box-shadow: var(--shadow-float);
		transform: none;
	}

	.phone {
		height: 100dvh;
	}

	.strip {
		display: flex;
		align-items: center;
		border-bottom: var(--border-width) solid var(--border);
		background: var(--surface-1);
	}

	[role='tablist'] {
		display: flex;
		flex: 1;
		align-self: stretch;
		min-width: 0;
		min-height: calc(var(--target-min) + var(--space-2));
	}

	.phone [role='tablist'] {
		overflow-x: auto;
		scrollbar-width: none;
	}

	.tabs {
		flex: 1;
		min-height: 0;
	}
</style>
