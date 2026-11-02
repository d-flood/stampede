<script lang="ts">
	import { LoadingMark } from '#lib/ui/spinners.ts';

	let { label }: { label: string } = $props();
</script>

<p role="status">
	<LoadingMark />
	{label}
</p>

<style>
	p {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		margin: 0;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}
</style>
