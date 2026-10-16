<script lang="ts">
	import Text from '#lib/chat/Text.svelte';
	import User from '#lib/chat/User.svelte';
	import type { Item } from '#lib/device.svelte.ts';

	let { items }: { items: Item[] } = $props();
</script>

{#each items as item, index (index)}
	{#if item.kind === 'user'}
		<User {item} />
	{:else if item.kind === 'text'}
		<Text {item} />
	{/if}
{/each}
