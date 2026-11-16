<script lang="ts">
	import { codeLines, diffLines, parser } from '#lib/chat/highlight.ts';
	import { highlight } from '#lib/editor/theme.ts';
	import { onMount } from 'svelte';
	import { StyleModule } from 'style-mod';

	let { code, path, diff = false, wrap = false }: { code: string; path: string; diff?: boolean; wrap?: boolean } = $props();
	const syntax = $derived(Promise.all([parser(path), /\.(sh|bash|zsh)$/.test(path) ? parser('inline.py') : undefined]).catch(() => []));

	onMount(() => {
		if (highlight.module) StyleModule.mount(document, highlight.module);
	});
</script>

{#snippet content(syntax?: Awaited<ReturnType<typeof parser>>, python?: Awaited<ReturnType<typeof parser>>)}
	{#if diff}
		{#each diffLines(code, syntax) as line, index (index)}
			<div class="code-line {line.kind}">{line.prefix}{#each line.tokens as token, index (index)}<span class={token.classes || undefined}>{token.text}</span>{/each}</div>
		{/each}
	{:else}
		<span class="syntax">{#each codeLines(code, syntax, python) as line, index (index)}{#if index}{'\n'}{/if}{#if wrap}<span class="wrap">{#each line as token, index (index)}<span class={token.classes || undefined}>{token.text}</span>{/each}</span>{:else}{#each line as token, index (index)}<span class={token.classes || undefined}>{token.text}</span>{/each}{/if}{/each}</span>
	{/if}
{/snippet}

{#await syntax}
	{@render content()}
{:then [syntax, python]}
	{@render content(syntax, python)}
{/await}

<style>
	:global(.syntax > .wrap) {
		display: inline-block;
		box-sizing: border-box;
		width: 100%;
		padding-left: var(--space-6);
		text-indent: calc(-1 * var(--space-6));
		vertical-align: top;
	}
</style>
