<script lang="ts">
	import { codeLines, diffLines, parser } from '#lib/chat/highlight.ts';
	import { highlight } from '#lib/editor/theme.ts';
	import { onMount } from 'svelte';
	import { StyleModule } from 'style-mod';

	let { code, path, diff = false }: { code: string; path: string; diff?: boolean } = $props();
	const syntax = $derived(parser(path).catch(() => undefined));

	onMount(() => {
		if (highlight.module) StyleModule.mount(document, highlight.module);
	});
</script>

{#snippet content(syntax?: Awaited<ReturnType<typeof parser>>)}
	{#if diff}
		{#each diffLines(code, syntax) as line, index (index)}
			<div class="code-line {line.kind}">{line.prefix}{#each line.tokens as token, index (index)}<span class={token.classes || undefined}>{token.text}</span>{/each}</div>
		{/each}
	{:else}
		<span class="syntax">{#each codeLines(code, syntax) as line, index (index)}{#if index}{'\n'}{/if}{#each line as token, index (index)}<span class={token.classes || undefined}>{token.text}</span>{/each}{/each}</span>
	{/if}
{/snippet}

{#await syntax}
	{@render content()}
{:then syntax}
	{@render content(syntax)}
{/await}
