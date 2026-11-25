<script lang="ts">
	import type { Device, ServerMessage, Terminal } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';
	import { warm } from '#lib/paint.svelte.ts';
	import CloseTerminal, { running } from '#lib/shell/CloseTerminal.svelte';
	import TerminalView from '#lib/shell/TerminalView.svelte';
	import Button from '#lib/ui/Button.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import Tab from '#lib/ui/Tab.svelte';
	import { onMount, tick } from 'svelte';
	import ArrowElbowDownLeft from '~icons/ph/arrow-elbow-down-left';
	import Clipboard from '~icons/ph/clipboard';
	import Copy from '~icons/ph/copy';
	import NotePencil from '~icons/ph/note-pencil';
	import Plus from '~icons/ph/plus';
	import TerminalWindow from '~icons/ph/terminal-window';
	import X from '~icons/ph/x';

	let { device, open = $bindable(false) }: { device: Device; open?: boolean } = $props();

	let panel: HTMLElement;
	let terminals = $state<Terminal[]>([]);
	let chosen = $state<string>();
	let closing = $state<Terminal>();
	const views = $state<Record<string, TerminalView>>({});
	let returnTo: HTMLElement | undefined;
	let height = $state<number>();
	let drag = $state<{ y: number; height: number }>();
	let viewport = $state(window.innerHeight);

	function resize(value: number) {
		height = Math.min(viewport - 48, Math.max(160, value));
	}

	function resizeKey(event: KeyboardEvent) {
		if (event.key === 'ArrowUp') resize(panel.offsetHeight - 32);
		else if (event.key === 'ArrowDown') resize(panel.offsetHeight + 32);
		else return;
		event.preventDefault();
	}

	const selectable = (terminal: Terminal) => terminal.state !== 'closing';
	const active = $derived(terminals.find((terminal) => terminal.id === chosen && selectable(terminal)) ?? terminals.findLast(selectable));

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
				if (message.terminal.owner === 'drawer') {
					if (!terminals.some((terminal) => terminal.id === message.terminal.id)) terminals.push(message.terminal);
					chosen = message.terminal.id;
				}
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

	export function ownsFocus() {
		return open && panel.contains(document.activeElement);
	}

	export function newTab() {
		device.send({ type: 'terminal.create', owner: 'drawer' });
	}

	export function closeActiveTab() {
		if (active) closeTab(active.id);
	}

	function keydown(event: KeyboardEvent) {
		if (event.code !== 'Backquote' || !event.ctrlKey || event.altKey || event.shiftKey || event.metaKey) return;
		event.preventDefault();
		event.stopPropagation();
		toggle();
	}

	function closeTab(terminalId: string, confirmed = false) {
		const terminal = terminals.find((terminal) => terminal.id === terminalId);
		if (!confirmed && terminal && running(terminal)) {
			closing = terminal;
			return;
		}
		closing = undefined;
		if (terminalId === active?.id) {
			const index = terminals.findIndex((terminal) => terminal.id === terminalId);
			const neighbor = terminals.slice(index + 1).find(selectable) ?? terminals.slice(0, index).findLast(selectable);
			if (neighbor) chosen = neighbor.id;
			else panel.focus();
		}
		device.send({ type: 'terminal.close', terminalId });
	}
</script>

<svelte:window onkeydowncapture={keydown} bind:innerHeight={viewport} />

<section aria-label="Drawer" tabindex="-1" class:open class:phone={form() === 'phone'} class:dragging={!!drag} inert={!open} bind:this={panel} style:height={form() === 'desktop' && height ? `${height}px` : undefined}>
	<div class="strip">
		<div role="tablist" aria-label="Drawer Tabs">
			{#each terminals as tab (tab.id)}
				{#if selectable(tab)}
					<Menu context items={[{ label: 'Close', onselect: () => closeTab(tab.id) }]}>
						{#snippet trigger(props)}
							<Tab
								{...props}
								variant="terminal"
								label={tab.label}
								ended={tab.state === 'ended'}
								active={tab.id === active?.id}
								onclose={() => closeTab(tab.id)}
								onclick={() => (chosen = tab.id)}
							/>
						{/snippet}
					</Menu>
				{:else}
					<Tab variant="terminal" label={tab.label} closing />
				{/if}
			{/each}
		</div>
		<Menu items={[{ label: 'Terminal', icon: TerminalWindow, onselect: newTab }]}>
			{#snippet trigger(props)}
				<IconButton {...props} label="New Drawer Tab" title="New tab (Ctrl/Cmd+T)"><Plus /></IconButton>
			{/snippet}
		</Menu>
		<span class="fill"></span>
		{#if form() === 'desktop' && active && views[active.id]}
			{@const controls = views[active.id].controls()}
			{#if controls.ended}
				<IconButton label="Copy mode" onclick={controls.copy}><Copy /></IconButton>
			{:else}
				<IconButton label="Paste" title="Paste" disabled={!controls.live} onclick={controls.paste}><Clipboard /></IconButton>
				<IconButton label="Newline" title="Insert newline (Shift+Enter)" disabled={!controls.live} onclick={controls.newline}><ArrowElbowDownLeft /></IconButton>
				<IconButton label="Composer" title="Composer" pressed={controls.composing} onclick={controls.compose}><NotePencil /></IconButton>
			{/if}
		{/if}
		{#if form() === 'phone'}
			<IconButton label="Close Drawer" onclick={toggle}><X /></IconButton>
		{/if}
	</div>
	<div class="tabs">
		{#if open || warm()}
			{#each terminals as terminal (terminal.id)}
				<TerminalView
					bind:this={views[terminal.id]}
					{device}
					terminalId={terminal.id}
					active={terminal.id === active?.id}
					displayed={open}
					focused={open && terminal.id === active?.id}
					toolbar={form() === 'phone'}
					ended={terminal.state === 'ended'}
				>
					{#if terminal.state === 'ended'}
						<div class="ended" role="status">
							<span>Lost to a Host reboot; showing its last output.</span>
							<Button onclick={() => closeTab(terminal.id)}>Dismiss</Button>
						</div>
					{/if}
				</TerminalView>
			{/each}
		{/if}
	</div>
	{#if form() === 'desktop'}
		<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions (Focusable separators support keyboard resizing.) -->
		<div
			class="grip"
			role="separator"
			aria-label="Resize Drawer"
			aria-orientation="horizontal"
			tabindex="0"
			onkeydown={resizeKey}
			onpointerdown={(event) => {
				if (event.button !== 0) return;
				event.preventDefault();
				drag = { y: event.clientY, height: panel.offsetHeight };
				event.currentTarget.setPointerCapture(event.pointerId);
			}}
			onpointermove={(event) => {
				if (drag && event.currentTarget.hasPointerCapture(event.pointerId)) resize(drag.height + event.clientY - drag.y);
			}}
			onpointerup={(event) => event.currentTarget.releasePointerCapture(event.pointerId)}
			onlostpointercapture={() => (drag = undefined)}
		></div>
	{/if}
</section>

<CloseTerminal bind:terminal={closing} onclose={(terminal) => closeTab(terminal.id, true)} />

<style>
	section {
		position: fixed;
		top: 0;
		right: 0;
		left: 0;
		z-index: 10;
		display: flex;
		flex-direction: column;
		height: 50dvh;
		border-bottom: var(--border-width) solid var(--border-strong);
		background: var(--color-bg);
		outline: none;
		transform: translateY(-100%);
		transition:
			transform var(--dur-med) ease-out,
			box-shadow var(--dur-med);
	}

	.open {
		box-shadow:
			var(--shadow-float),
			0 0 0 100vmax color-mix(in oklab, var(--ansi-0) 35%, transparent);
		transform: none;
	}

	.dragging {
		transition: none;
	}

	.grip {
		position: absolute;
		right: 0;
		bottom: 0;
		left: 0;
		z-index: 1;
		display: grid;
		place-items: center;
		height: var(--space-3);
		cursor: row-resize;
		touch-action: none;
		user-select: none;
	}

	.grip::after {
		content: '';
		width: calc(2 * var(--tile));
		height: var(--space-1);
		border-radius: var(--radius-pill);
		background: var(--border-strong);
	}

	.grip:hover::after,
	.grip:focus-visible::after,
	.dragging .grip::after {
		background: var(--muted-fg);
	}

	.grip:focus-visible {
		outline: none;
	}

	section:not(.phone) {
		border-radius: 0 0 var(--radius-xl) var(--radius-xl);
		overflow: clip;
	}

	.phone {
		height: 100dvh;
	}

	.strip {
		display: flex;
		align-items: center;
		padding: 0 var(--space-1);
		border-bottom: var(--border-width) solid var(--border);
		background: var(--surface-1);
	}

	[role='tablist'] {
		display: flex;
		flex: 0 1 auto;
		align-self: stretch;
		align-items: flex-end;
		gap: 2px;
		min-width: 0;
		min-height: calc(var(--target-min) + var(--space-2));
		padding-top: var(--space-2);
	}

	.fill {
		flex: 1;
	}

	.phone [role='tablist'] {
		overflow-x: auto;
		scrollbar-width: none;
	}

	.ended {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
		padding: var(--space-2);
		border-bottom: var(--border-width) solid var(--border);
		color: var(--muted-fg);
	}

	.tabs {
		flex: 1;
		min-height: 0;
	}
</style>
