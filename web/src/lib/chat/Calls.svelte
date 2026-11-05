<script lang="ts">
	import { canvasName } from '#lib/canvas.svelte.ts';
	import { brief } from '#lib/chat/brief.ts';
	import Code from '#lib/chat/Code.svelte';
	import Command from '#lib/chat/Command.svelte';
	import Edit from '#lib/chat/Edit.svelte';
	import { icon, icons } from '#lib/chat/icons.ts';
	import { chips, summarize, type Chip } from '#lib/chat/summary.ts';
	import Thinking from '#lib/chat/Thinking.svelte';
	import type { Item, ToolCall } from '#lib/device.svelte.ts';
	import CaretRight from '~icons/ph/caret-right';

	let { items, activity, open = false, ondiff, oncanvas, onexpand }: { items: Item[]; activity?: string | null; open?: boolean; ondiff(path: string): void; oncanvas(canvas: string): void; onexpand?(item: Item): void } = $props();

	let expanded = $derived(open);
	let shown = $state<Record<number, boolean>>({});
	const segments = $derived(summarize(items));
	const edits = $derived(chips(items));
	const running = $derived(items.flatMap((item) => (item.kind === 'command' && activity === `Running ${item.command}` ? [item.command] : []))[0]);
	const verbs: Partial<Record<ToolCall['kind'], string>> = { read: 'Read', search: 'Search', fetch: 'Fetch', web: 'Web search' };
	const ROW_CHARACTERS = 500;

	function dot(item: Item) {
		if (item.kind === 'thinking') return 'said';
		if (item.kind === 'command') return item.status;
		return 'done';
	}
</script>

{#snippet chip(chip: Chip)}
	<span class="name">{chip.canvas ? canvasName(chip.path) : chip.path.split('/').at(-1)}</span><span class="added">+{chip.added}</span><span class="removed">−{chip.removed}</span>
{/snippet}

{#snippet marker(Icon: ReturnType<typeof icon>)}
	{#if Icon}<span class="marker"><Icon /></span>{/if}
{/snippet}

<div class="calls">
	<button class="summary" aria-expanded={expanded} onclick={() => (expanded = !expanded)}>
		<span class="caret"><CaretRight /></span><span class="text"><span class="segments">{#each segments as segment, index (index)}{#if index}{', '}{/if}<span class="segment">{segment}</span>{/each}</span>{#if activity}<span class="live" title={activity}>{#if running}{' · Running '}<code><Code code={brief(running).slice(0, ROW_CHARACTERS)} path="command.sh" /></code>{:else}{` · ${brief(activity)}`}{/if}</span>{/if}</span>
	</button>
	{#if edits.length}
		<div class="chips">
			{#each edits as edit (`${edit.canvas}:${edit.path}`)}
				{#if edit.canvas}
					<button class="chip" title={edit.path} onclick={() => oncanvas(edit.path)}>{@render chip(edit)}</button>
				{:else if edit.path.startsWith('/')}
					<span class="chip" title={edit.path}>{@render chip(edit)}</span>
				{:else}
					<button class="chip" title={edit.path} onclick={() => ondiff(edit.path)}>{@render chip(edit)}</button>
				{/if}
			{/each}
		</div>
	{/if}
	{#if expanded}
		<div class="rail">
			{#each items as item, index (index)}
				{#if item.kind === 'tools'}
					{#each item.calls as call, number (number)}
						<div class="step" data-dot="done">
							{@render marker(icons[call.kind])}
							<div class="call"><span class="verb">{verbs[call.kind] ?? call.name}</span> <span class="detail" title={call.detail}>{#if verbs[call.kind]}{call.detail}{:else}<Code code={call.detail} path="arguments.json" />{/if}</span></div>
						</div>
					{/each}
				{:else}
					<div class="step" data-dot={dot(item)}>
						{@render marker(icon(item))}
						{#if item.kind === 'thinking'}
							<Thinking {item} />
						{:else if item.kind === 'command'}
							<Command {item} bind:open={() => shown[index] ?? false, (value) => (shown[index] = value)} onexpand={onexpand && (() => onexpand(item))} />
						{:else if item.kind === 'edit'}
							<Edit {item} {ondiff} />
						{:else if item.kind === 'canvas'}
							<div class="call"><span class="verb">Edited canvas</span> <span class="detail">{canvasName(item.canvas)}</span><span class="added">+{item.added}</span><span class="removed">−{item.removed}</span><button class="open" onclick={() => oncanvas(item.canvas)}>Open</button></div>
						{/if}
					</div>
				{/if}
			{/each}
		</div>
	{/if}
</div>

<style>
	.calls {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		font-family: var(--font-ui);
	}

	.summary {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-width: 0;
		min-height: var(--target-min);
		padding: 0;
		border: 0;
		background: none;
		color: var(--muted-fg);
		font: inherit;
		font-size: var(--font-size-sm);
		text-align: start;
		cursor: pointer;
	}

	.summary:hover {
		color: var(--color-fg);
	}

	.caret {
		display: inline-flex;
		flex: none;
		width: 1em;
		transition: transform var(--dur-fast);
	}

	[aria-expanded='true'] > .caret {
		transform: rotate(90deg);
	}

	.text {
		display: flex;
		flex: 1;
		flex-wrap: wrap;
		column-gap: var(--space-1);
		min-width: 0;
	}

	.segment {
		white-space: nowrap;
	}

	.live {
		flex: 1 1 12rem;
		min-width: 0;
		overflow: hidden;
		color: var(--muted-fg);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.live code {
		font-family: var(--font-mono);
	}

	.chips {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}

	.chip {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		min-width: 0;
		max-width: 100%;
		min-height: var(--target-min);
		padding: 0 var(--space-3);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--color-bg);
		color: var(--color-fg);
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
	}

	button.chip {
		cursor: pointer;
	}

	button.chip:hover {
		background: var(--surface-1);
	}

	.chip .name {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.added {
		color: var(--git-added);
	}

	.removed {
		color: var(--git-deleted);
	}

	.open {
		flex: none;
		min-height: var(--target-min);
		margin-left: auto;
		padding: 0;
		border: 0;
		background: none;
		color: var(--accent);
		font: inherit;
		cursor: pointer;
	}

	.open:hover {
		text-decoration: underline;
	}

	.rail {
		--indent: var(--space-6);
		--dot: 9px;
		--marker: 14px;
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		margin-top: var(--space-1);
		margin-left: var(--space-1);
		padding-left: var(--indent);
		border-left: var(--border-width) solid var(--border);
		font-size: var(--font-size-sm);
	}

	:global(.phone) .rail {
		--indent: var(--space-4);
	}

	.step {
		position: relative;
		min-width: 0;
	}

	.call {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-height: var(--target-min);
		color: var(--muted-fg);
	}

	.verb {
		flex: none;
		color: var(--color-fg);
	}

	.detail {
		min-width: 0;
		overflow: hidden;
		font-family: var(--font-mono);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	[data-dot='said']::before {
		content: '';
		position: absolute;
		top: calc(var(--target-min) / 2 - var(--dot) / 2);
		left: calc(-1 * var(--indent) - var(--border-width) / 2 - var(--dot) / 2);
		box-sizing: border-box;
		width: var(--dot);
		height: var(--dot);
		border: var(--border-width) solid var(--muted-fg);
		border-radius: 50%;
		background: var(--color-bg);
	}

	.marker {
		position: absolute;
		top: calc(var(--target-min) / 2 - var(--marker) / 2);
		left: calc(-1 * var(--indent) - var(--border-width) / 2 - var(--marker) / 2);
		display: flex;
		width: var(--marker);
		height: var(--marker);
		background: var(--color-bg);
		color: var(--muted-fg);
	}

	.marker :global(svg) {
		width: 100%;
		height: 100%;
	}

	[data-dot='ok'] > .marker {
		color: var(--status-done);
	}

	[data-dot='failed'] > .marker {
		color: var(--ansi-1);
	}

	[data-dot='running'] > .marker {
		color: var(--status-working);
		animation: fade 1.6s ease-in-out var(--pulse);
	}

	@keyframes fade {
		50% {
			opacity: 0.3;
		}
	}
</style>
