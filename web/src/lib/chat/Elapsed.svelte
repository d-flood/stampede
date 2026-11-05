<script lang="ts">
	import { clock } from '#lib/chat/agents.ts';

	let { startedAt }: { startedAt: number } = $props();
</script>

<span class="elapsed">{clock(startedAt)}</span>

<style>
	.elapsed { flex: none; color: var(--muted-fg); font-family: var(--font-ui); font-variant-numeric: tabular-nums; white-space: nowrap; }
</style>
