<script lang="ts">
	import Code from '#lib/chat/Code.svelte';
	import type { Item, ToolCall } from '#lib/device.svelte.ts';

	let { item }: { item: Extract<Item, { kind: 'tools' }> } = $props();

	const verbs = { read: 'Read', search: 'Search', fetch: 'Fetch', web: 'Web search' };

	const phrases: [ToolCall['kind'], (count: number) => string][] = [
		['read', (count) => `read ${count} ${count === 1 ? 'file' : 'files'}`],
		['search', (count) => `searched ${count} ${count === 1 ? 'pattern' : 'patterns'}`],
		['fetch', (count) => `fetched ${count} ${count === 1 ? 'page' : 'pages'}`],
		['web', (count) => (count === 1 ? 'searched the web' : `searched the web ${count} times`)]
	];

	const summary = $derived.by(() => {
		const parts = phrases.flatMap(([kind, phrase]) => {
			const count = item.calls.filter((call) => call.kind === kind).length;
			return count ? [phrase(count)] : [];
		});
		const others = new Set(item.calls.filter((call) => call.kind === 'other').map((call) => call.name));
		if (others.size) parts.push(`used ${[...others].join(', ')}`);
		const text = parts.join(' · ');
		return text.charAt(0).toUpperCase() + text.slice(1);
	});
</script>

<details>
	<summary><span>{summary}</span></summary>
	<ul>
		{#each item.calls as call, index (index)}
			<li><span class="verb">{call.kind === 'other' ? call.name : verbs[call.kind]}</span> {#if call.kind === 'other'}<Code code={call.detail} path="arguments.json" />{:else}{call.detail}{/if}</li>
		{/each}
	</ul>
</details>

<style>
	details {
		color: var(--muted-fg);
		font-family: var(--font-mono);
		font-size: 0.92em;
	}

	summary {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-height: var(--target-min);
		list-style: none;
		cursor: pointer;
	}

	summary::-webkit-details-marker {
		display: none;
	}

	summary::before {
		content: '▸';
	}

	details[open] > summary::before {
		content: '▾';
	}

	summary:hover {
		color: var(--color-fg);
	}

	summary span {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	ul {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		margin: 0;
		padding: 0 0 0 var(--space-4);
		list-style: none;
	}

	li {
		overflow-wrap: anywhere;
	}

	.verb {
		color: var(--color-fg);
	}
</style>
