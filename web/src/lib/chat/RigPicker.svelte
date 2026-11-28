<script lang="ts">
	import Chip from '#lib/ui/Chip.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import type { Component } from 'svelte';
	import CaretDown from '~icons/ph/caret-down';

	type Rig = { label: string; icon?: Component; shortcut?: string };

	let { rigs, current, onpick }: { rigs: Rig[]; current: number; onpick(index: number): void } = $props();

	const rig = $derived(rigs[current]);
	const items = $derived([{ heading: 'Rigs', items: rigs.map((entry, index) => ({ label: entry.label, icon: entry.icon, shortcut: entry.shortcut, checked: index === current, onselect: () => onpick(index) })) }]);
</script>

{#if rig}
	{@const Logo = rig.icon}
	<span class="rig-picker">
		<Menu side="top" align="start" {items}>
			{#snippet trigger(props)}
				<Chip {...props} variant="ghost" aria-label="Rig: {rig.label}">
					{#if Logo}<span class="mark" aria-hidden="true"><Logo /></span>{/if}
					<span class="name">{rig.label}</span>
					<span class="caret"><CaretDown aria-hidden="true" /></span>
				</Chip>
			{/snippet}
		</Menu>
	</span>
{/if}

<style>
	.rig-picker {
		display: flex;
		flex: 0 1 auto;
		min-width: 0;
	}

	.rig-picker :global(.chip) {
		color: var(--color-fg);
	}

	.mark {
		display: inline-flex;
		flex: none;
		align-items: center;
		margin-right: var(--space-1);
	}

	.name {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.caret {
		display: inline-flex;
	}

	.caret :global(svg) {
		width: 0.85em;
		height: 0.85em;
	}
</style>
