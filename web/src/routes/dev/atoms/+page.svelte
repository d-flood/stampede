<script lang="ts">
	import { connect } from '#lib/device.svelte.ts';
	import Badge from '#lib/ui/Badge.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import Section from '#lib/ui/Section.svelte';
	import Tab from '#lib/ui/Tab.svelte';
	import Tooltip from '#lib/ui/Tooltip.svelte';
	import { onMount } from 'svelte';
	import DotsThree from '~icons/ph/dots-three';
	import Gear from '~icons/ph/gear';
	import Plus from '~icons/ph/plus';
	import X from '~icons/ph/x';

	onMount(() => connect().close);

	const colors = [
		'--color-bg',
		'--color-fg',
		'--color-cursor',
		'--color-selection-bg',
		'--color-selection-fg',
		'--surface-1',
		'--surface-2',
		'--surface-3',
		'--border',
		'--hover',
		'--muted-fg',
		'--status-working',
		'--status-waiting',
		'--status-done',
		'--status-background',
		...Array.from({ length: 16 }, (_, n) => `--ansi-${n}`)
	];
	const statuses = [undefined, 'working', 'waiting', 'background', 'done'] as const;

	let open = $state(false);
	let chosen = $state('nothing yet');
</script>

<main>
	<h1>Atoms</h1>

	<Section title="Colors">
		<div class="swatches">
			{#each colors as color (color)}
				<figure>
					<div class="swatch" style:background="var({color})"></div>
					<figcaption>{color}</figcaption>
				</figure>
			{/each}
		</div>
	</Section>

	<Section title="Fonts">
		<p class="mono">JetBrains Mono: 0O 1lI {'{}'} =&gt; != <b>bold</b> <i>italic</i></p>
		<p class="mono">Nerd symbols: {'\uf07b \ue725 \uf09b \ue0b0 \u{f02a2}'}</p>
		<p>The same font throughout the UI</p>
	</Section>

	<Section title="Button">
		<div class="row">
			<Button>Default</Button>
			<Button variant="primary">Primary</Button>
			<Button disabled>Disabled</Button>
			<Button variant="primary" disabled>Primary disabled</Button>
			<Button variant="ghost">Ghost</Button>
			<Button variant="danger">Danger</Button>
			<Button><Plus /> With icon</Button>
		</div>
	</Section>

	<Section title="IconButton">
		<div class="row">
			<IconButton label="Add"><Plus /></IconButton>
			<IconButton label="Settings"><Gear /></IconButton>
			<IconButton label="Close" disabled><X /></IconButton>
		</div>
	</Section>

	<Section title="Tab">
		{#each [false, true] as active (active)}
			<div class="tabs" role="tablist">
				{#each statuses as status (status)}
					<Tab variant="agent" label="claude-yolo" {status} {active} />
				{/each}
				<Tab variant="agent" agent="codex" label="codex" status="working" {active} />
				<Tab variant="terminal" label="zsh ~/repos" {active} />
				<Tab variant="editor" label="device.ts" {active} />
				<Tab variant="diff" label="lib.rs" {active} />
			</div>
		{/each}
	</Section>

	<Section title="Badge">
		<div class="row">
			<Badge>3</Badge>
			<Badge tone="working">1</Badge>
			<Badge tone="waiting">2</Badge>
			<Badge tone="done">12</Badge>
		</div>
	</Section>

	<Section title="Menu">
		<div class="row">
			<Menu
				items={[
					{ label: 'claude-yolo', onselect: () => (chosen = 'claude-yolo') },
					{ label: 'Terminal', onselect: () => (chosen = 'Terminal') },
					{ label: 'Open file', onselect: () => (chosen = 'Open file'), disabled: true }
				]}
			>
				{#snippet trigger(props)}
					<IconButton label="New tab" {...props}><Plus /></IconButton>
				{/snippet}
			</Menu>
			<span>Chosen: {chosen}</span>
		</div>
	</Section>

	<Section title="Dialog">
		<Button onclick={() => (open = true)}>Close Project…</Button>
		<Dialog bind:open title="Close stampede?" description="An Agent is working. Closing ends its Terminal.">
			{#snippet actions()}
				<Button onclick={() => (open = false)}>Cancel</Button>
				<Button variant="primary" onclick={() => (open = false)}>Close Project</Button>
			{/snippet}
		</Dialog>
	</Section>

	<Section title="Tooltip">
		<Tooltip text="Settings">
			{#snippet children(props)}
				<IconButton label="Settings" {...props}><Gear /></IconButton>
			{/snippet}
		</Tooltip>
	</Section>

	<Section title="Section">
		{#snippet actions()}
			<IconButton label="More"><DotsThree /></IconButton>
		{/snippet}
		<p>A Section holds a title, optional actions, and its content.</p>
	</Section>
</main>

<style>
	main {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: var(--space-4);
		max-width: 60rem;
		margin: 0 auto;
		padding: var(--space-4);
	}

	h1 {
		margin: 0;
		font-size: var(--font-size-lg);
	}

	p {
		margin: 0 0 var(--space-2);
	}

	.mono {
		font-family: var(--font-mono);
	}

	.row {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
	}

	.tabs {
		display: flex;
		overflow: auto hidden;
		border-bottom: var(--border-width) solid var(--border);
		background: var(--surface-1);
	}

	.swatches {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(8rem, 1fr));
		gap: var(--space-2);
	}

	figure {
		margin: 0;
	}

	.swatch {
		height: 2.5rem;
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-sm);
	}

	figcaption {
		color: var(--muted-fg);
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
	}
</style>
