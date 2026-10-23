<script lang="ts">
	import type { Subagent } from '#lib/device.svelte.ts';

	let { origin }: { origin: Subagent['origin'] } = $props();
</script>

<span class="origin" title="Spawned by">{origin === 'stampede' ? 'Stampede delegate' : 'Harness'}</span>

<style>
	.origin { flex: none; padding: 0 var(--space-1); border-radius: var(--radius-sm); background: var(--surface-3); color: var(--muted-fg); font-size: var(--font-size-sm); white-space: nowrap; }
</style>
