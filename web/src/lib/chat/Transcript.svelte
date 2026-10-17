<script lang="ts">
	import Command from '#lib/chat/Command.svelte';
	import Edit from '#lib/chat/Edit.svelte';
	import Html from '#lib/chat/Html.svelte';
	import Notice from '#lib/chat/Notice.svelte';
	import Subagent from '#lib/chat/Subagent.svelte';
	import Text from '#lib/chat/Text.svelte';
	import Thinking from '#lib/chat/Thinking.svelte';
	import Tools from '#lib/chat/Tools.svelte';
	import User from '#lib/chat/User.svelte';
	import type { Item } from '#lib/device.svelte.ts';

	let { items, ondiff }: { items: Item[]; ondiff(path: string): void } = $props();
</script>

{#each items as item, index (index)}
	{#if item.kind === 'user'}
		<User {item} first={index === 0} />
	{:else if item.kind === 'text'}
		<Text {item} />
	{:else if item.kind === 'thinking'}
		<Thinking {item} />
	{:else if item.kind === 'tools'}
		<Tools {item} />
	{:else if item.kind === 'html'}
		<Html {item} />
	{:else if item.kind === 'edit'}
		<Edit {item} {ondiff} />
	{:else if item.kind === 'command'}
		<Command {item} />
	{:else if item.kind === 'subagent'}
		<Subagent {item} {ondiff} />
	{:else if item.kind === 'notice'}
		<Notice {item} />
	{/if}
{/each}
