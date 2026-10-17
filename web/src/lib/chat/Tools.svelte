<script lang="ts">
	import Code from '#lib/chat/Code.svelte';
	import { icons } from '#lib/chat/icons.ts';
	import type { Item, ToolCall } from '#lib/device.svelte.ts';
	import CaretRight from '~icons/ph/caret-right';

	let { item }: { item: Extract<Item, { kind: 'tools' }> } = $props();

	const verbs: Partial<Record<ToolCall['kind'], string>> = { read: 'Read', search: 'Search', fetch: 'Fetch', web: 'Web search' };

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
		const others = new Set(item.calls.filter((call) => !verbs[call.kind]).map((call) => call.name));
		if (others.size) parts.push(`used ${[...others].join(', ')}`);
		const text = parts.join(' · ');
		return text.charAt(0).toUpperCase() + text.slice(1);
	});
</script>

<details>
	<summary><span class="caret"><CaretRight /></span><span class="text">{summary}</span></summary>
	<ul>
		{#each item.calls as call, index (index)}
			{@const Icon = icons[call.kind]}
			<li><span class="icon"><Icon /></span><span><span class="verb">{verbs[call.kind] ?? call.name}</span> {#if verbs[call.kind]}{call.detail}{:else}<Code code={call.detail} path="arguments.json" />{/if}</span></li>
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

	.caret {
		display: inline-flex;
		flex: none;
		width: 1em;
		transition: transform var(--dur-fast);
	}

	details[open] > summary > .caret {
		transform: rotate(90deg);
	}

	summary:hover {
		color: var(--color-fg);
	}

	summary .text {
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
		display: flex;
		gap: var(--space-2);
		overflow-wrap: anywhere;
	}

	li > span {
		min-width: 0;
	}

	.icon {
		display: inline-flex;
		flex: none;
		height: 1lh;
		align-items: center;
	}

	.verb {
		color: var(--color-fg);
	}
</style>
