<script lang="ts">
	import { codeLines, parser } from '#lib/chat/highlight.ts';
	import { read } from '#lib/files.svelte.ts';
	import { Portal } from 'bits-ui';
	import { innerHeight } from 'svelte/reactivity/window';

	let { projectId, path, line, anchor }: { projectId: string; path: string; line?: number; anchor: DOMRect } = $props();

	const excerpt = $derived(
		Promise.all([read(projectId, path), parser(path).catch(() => undefined)]).then(([contents, syntax]) => {
			if (contents.binary) return;
			const lines = contents.content.split('\n');
			const target = Math.min(Math.max(line ?? 1, 1), lines.length);
			const first = Math.max(1, target - 4);
			return { first, target, lines: codeLines(lines.slice(first - 1, first + 9).join('\n'), syntax) };
		})
	);
	const height = $derived(innerHeight.current ?? 0);
	const below = $derived(height - anchor.bottom >= anchor.top);
</script>

<Portal>
	<div
		class="preview"
		role="tooltip"
		style:--left="{anchor.left}px"
		style:top={below ? `${anchor.bottom + 4}px` : undefined}
		style:bottom={below ? undefined : `${height - anchor.top + 4}px`}
		style:max-height="{(below ? height - anchor.bottom : anchor.top) - 12}px"
	>
		<div class="path">{path}{line ? `:${line}` : ''}</div>
		{#await excerpt then excerpt}
			{#if excerpt}
				<div class="code">
					{#each excerpt.lines as tokens, index (index)}
						{@const number = excerpt.first + index}
						<div class="line" class:target={line && number === excerpt.target}>
							<span class="number">{number}</span>
							<span class="text">{#each tokens as token, index (index)}<span class={token.classes || undefined}>{token.text}</span>{/each}</span>
						</div>
					{/each}
				</div>
			{:else}
				<div class="note">Binary file</div>
			{/if}
		{:catch}
			<div class="note">Couldn’t read this file</div>
		{/await}
	</div>
</Portal>

<style>
	.preview {
		position: fixed;
		--width: min(40rem, calc(100vw - 16px));
		z-index: 50;
		left: clamp(8px, var(--left), calc(100vw - var(--width) - 8px));
		width: var(--width);
		overflow: hidden;
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-1);
		box-shadow: var(--shadow-float);
		pointer-events: none;
	}

	.path,
	.note {
		padding: var(--space-1) var(--space-3);
		color: var(--muted-fg);
		font-family: var(--font-ui);
		font-size: var(--font-size-sm);
	}

	.path {
		overflow: hidden;
		border-bottom: var(--border-width) solid var(--border);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.code {
		padding: var(--space-1) 0;
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		line-height: 1.5;
		tab-size: 4;
	}

	.line {
		display: flex;
		gap: var(--space-3);
		padding: 0 var(--space-3);
		white-space: pre;
	}

	.target {
		background: color-mix(in oklab, var(--accent) 16%, transparent);
	}

	.number {
		flex: none;
		min-width: 3ch;
		color: var(--muted-fg);
		text-align: end;
	}

	.text {
		overflow: hidden;
		text-overflow: ellipsis;
	}
</style>
