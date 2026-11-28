<script lang="ts">
	/** `[start, end)` offsets into `text`, ascending and disjoint. */
	let { text, ranges }: { text: string; ranges: [number, number][] } = $props();

	const runs = $derived.by(() => {
		const runs: { text: string; mark: boolean }[] = [];
		let at = 0;
		for (const [start, end] of ranges) {
			if (start > at) runs.push({ text: text.slice(at, start), mark: false });
			if (end > start) runs.push({ text: text.slice(start, end), mark: true });
			at = Math.max(at, end);
		}
		if (at < text.length) runs.push({ text: text.slice(at), mark: false });
		return runs;
	});
</script>

{#each runs as run, index (index)}{#if run.mark}<mark>{run.text}</mark>{:else}{run.text}{/if}{/each}

<style>
	mark {
		border-radius: var(--radius-sm);
		background: var(--match-bg);
		color: var(--color-fg);
		box-decoration-break: clone;
	}
</style>
