<script lang="ts">
	import CaretRight from '~icons/ph/caret-right';

	let { prompt }: { prompt: string } = $props();

	let open = $state(false);
	let long = $state(false);

	function measure(element: HTMLElement) {
		const observer = new ResizeObserver(() => {
			if (!open) long = element.scrollHeight > element.clientHeight;
		});
		observer.observe(element);
		return () => observer.disconnect();
	}
</script>

<section class="prompt" aria-label="Prompt">
	<span class="label">Prompt</span>
	<div class="text" class:open {@attach measure}>{prompt}</div>
	{#if long}
		<button class="toggle" aria-expanded={open} onclick={() => (open = !open)}><span class="caret"><CaretRight /></span>{open ? 'Show less' : 'Show more'}</button>
	{/if}
</section>

<style>
	.prompt { display: flex; flex-direction: column; align-items: flex-start; gap: var(--space-1); padding: var(--space-3) var(--space-4); border: var(--border-width) solid var(--border); border-radius: var(--radius-lg); background: var(--surface-1); }
	.label { color: var(--muted-fg); font-size: var(--font-size-sm); }
	.text { display: -webkit-box; overflow: hidden; -webkit-box-orient: vertical; -webkit-line-clamp: 2; line-clamp: 2; white-space: pre-wrap; overflow-wrap: anywhere; }
	.text.open { display: block; }
	.toggle { display: inline-flex; align-items: center; gap: var(--space-1); min-height: var(--target-min); padding: 0; border: none; background: none; color: var(--muted-fg); font: inherit; font-size: var(--font-size-sm); cursor: pointer; }
	.toggle:hover { color: var(--color-fg); }
	.caret { display: inline-flex; transition: transform var(--dur-fast); }
	[aria-expanded='true'] .caret { transform: rotate(90deg); }
</style>
