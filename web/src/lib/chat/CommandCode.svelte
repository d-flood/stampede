<script lang="ts">
	import CodeBlock from '#lib/chat/CodeBlock.svelte';
	import { formatCommand } from '#lib/chat/format-command.ts';

	let { code }: { code: string } = $props();
	const formatted = $derived(formatCommand(code));
</script>

<div class="command-code">
	<CodeBlock {code} {formatted} path="command.sh" />
</div>

<style>
	.command-code {
		min-width: 0;
	}
</style>
