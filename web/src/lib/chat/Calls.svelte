<script lang="ts">
	import { canvasName } from '#lib/canvas.svelte.ts';
	import Approval from '#lib/chat/Approval.svelte';
	import { brief } from '#lib/chat/brief.ts';
	import Code from '#lib/chat/Code.svelte';
	import CodeBlock from '#lib/chat/CodeBlock.svelte';
	import Command from '#lib/chat/Command.svelte';
	import Edit from '#lib/chat/Edit.svelte';
	import Elapsed from '#lib/chat/Elapsed.svelte';
	import { formatJson } from '#lib/chat/format-json.ts';
	import { icon, toolIcon } from '#lib/chat/icons.ts';
	import { chips, summarize, type Chip as Edited } from '#lib/chat/summary.ts';
	import Thinking from '#lib/chat/Thinking.svelte';
	import Tool from '#lib/chat/Tool.svelte';
	import type { Item, ToolCall } from '#lib/device.svelte.ts';
	import Button from '#lib/ui/Button.svelte';
	import Caret from '#lib/ui/Caret.svelte';
	import Chip from '#lib/ui/Chip.svelte';

	let { items, activity, open = false, ondiff, oncanvas, onexpand }: { items: Item[]; activity?: string | null; open?: boolean; ondiff(path: string): void; oncanvas(canvas: string): void; onexpand?(item: Item): void } = $props();

	let expanded = $derived(open);
	let watching = $state(false);
	let shown = $state<Record<number, boolean>>({});
	let shownTools = $state<Record<string, boolean>>({});
	const segments = $derived(summarize(items));
	const edits = $derived(chips(items));
	const running = $derived(items.flatMap((item) => (item.kind === 'command' && activity === `Running ${item.command}` ? [item.command] : []))[0]);
	const current = $derived(items.at(-1));
	// Codex starts a command before its approval review lands, so the verdict can trail the running command.
	const latest = $derived(items.findLast((item) => item.kind !== 'tools' || item.calls.some((call) => call.kind !== 'approval')));
	const command = $derived(latest?.kind === 'command' && activity === `Running ${latest.command}` ? latest : undefined);
	const tool = $derived(current?.kind === 'tools' && activity === `Using ${current.calls.at(-1)?.name}` ? current.calls.at(-1) : undefined);
	const args = $derived(watching && tool ? formatJson(tool.detail) : undefined);
	const rail = $derived(watching && command ? items.filter((item) => item !== command) : items);
	const verbs: Partial<Record<ToolCall['kind'], string>> = { read: 'Read', search: 'Search', fetch: 'Fetch', web: 'Web search' };
	const ROW_CHARACTERS = 500;

	function toggle() {
		expanded = !expanded;
		const commands = rail.flatMap((item, index) => (item.kind === 'command' ? [index] : []));
		const index = rail.findLastIndex((item) => item.kind === 'command' && item.status === 'running');
		if (expanded && (index >= 0 || commands.length === 1)) shown[index >= 0 ? index : commands[0]] = true;
	}

	function dot(item: Item) {
		if (item.kind === 'thinking') return 'said';
		if (item.kind === 'command') return item.status;
		return 'done';
	}
</script>

{#snippet chip(chip: Edited)}
	<span class="name">{chip.canvas ? canvasName(chip.path) : chip.path.split('/').at(-1)}</span>{#if chip.added}<span class="added">+{chip.added}</span>{/if}{#if chip.removed}<span class="removed">−{chip.removed}</span>{/if}
{/snippet}

{#snippet marker(Icon: ReturnType<typeof icon>)}
	{#if Icon}<span class="marker"><Icon /></span>{/if}
{/snippet}

<div class="calls">
	<div class="head">
		<button class="summary" aria-expanded={expanded} onclick={toggle}>
			<Caret /><span class="text"><span class="segments">{#each segments as segment, index (index)}{#if index}{', '}{/if}<span class="segment">{segment}</span>{/each}</span>{#if activity && !command && !tool}<span class="live" title={activity}>{#if running}{' · Running '}<code><Code code={brief(running).slice(0, ROW_CHARACTERS)} path="command.sh" /></code>{:else}{` · ${brief(activity)}`}{/if}</span>{/if}</span>
		</button>
		{#if activity && (command || tool)}
			<button class="live" aria-expanded={watching} title={activity} onclick={() => (watching = !watching)}>
				<span class="label">{#if command && !watching}{' · Running '}<code><Code code={brief(command.command).slice(0, ROW_CHARACTERS)} path="command.sh" /></code>{:else}{` · ${command ? 'Running' : brief(activity)}`}{/if}</span>{#if command?.startedAt && !watching}<Elapsed startedAt={command.startedAt} />{/if}<Caret />
			</button>
		{/if}
	</div>
	{#if edits.length}
		<div class="chips">
			{#each edits as edit (`${edit.canvas}:${edit.path}`)}
				{#if edit.canvas}
					<Chip title={edit.path} onclick={() => oncanvas(edit.path)}>{@render chip(edit)}</Chip>
				{:else if edit.path.startsWith('/')}
					<Chip element="span" title={edit.path}>{@render chip(edit)}</Chip>
				{:else}
					<Chip title={edit.path} onclick={() => ondiff(edit.path)}>{@render chip(edit)}</Chip>
				{/if}
			{/each}
		</div>
	{/if}
	{#if expanded}
		<div class="rail">
			{#each rail as item, index (index)}
				{#if item.kind === 'tools'}
					{#each (watching && tool && index === items.length - 1 ? item.calls.slice(0, -1) : item.calls) as call, number (number)}
						<div class="step" data-dot={call.kind === 'approval' && call.name !== 'approved' ? 'failed' : 'done'}>
							{@render marker(toolIcon(call))}
							{#if call.kind === 'mcp' || call.kind === 'other'}
								<Tool {call} bind:open={() => shownTools[`${index}:${number}`] ?? false, (value) => (shownTools[`${index}:${number}`] = value)} />
							{:else if call.kind === 'approval'}
								<Approval {call} bind:open={() => shownTools[`${index}:${number}`] ?? false, (value) => (shownTools[`${index}:${number}`] = value)} />
							{:else}
								<div class="call"><span class="verb">{verbs[call.kind] ?? call.name}</span> <span class="detail" title={call.detail}>{call.detail}</span></div>
							{/if}
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
							<div class="call"><span class="verb">Edited canvas</span> <span class="detail">{canvasName(item.canvas)}</span>{#if item.added}<span class="added">+{item.added}</span>{/if}{#if item.removed}<span class="removed">−{item.removed}</span>{/if}<Button variant="link" onclick={() => oncanvas(item.canvas)}>Open</Button></div>
						{/if}
					</div>
				{/if}
			{/each}
		</div>
	{/if}
	{#if watching && command}
		<div class="current-command">
			<Command item={command} full bind:open={() => shown[items.indexOf(command)] ?? false, (value) => (shown[items.indexOf(command)] = value)} onexpand={onexpand && (() => onexpand(command))} />
		</div>
	{:else if watching && tool}
		<div class="current-tool">
			<CodeBlock code={tool.detail} formatted={args ?? tool.detail} path={args === undefined ? 'arguments.txt' : 'arguments.json'} />
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

	/* Hang the Caret in the padding so a wrapped live button aligns with the segments. */
	.head {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		column-gap: var(--space-1);
		min-width: 0;
		padding-left: calc(1em + var(--space-2));
		font-size: var(--font-size-md);
	}

	.summary,
	button.live {
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
		text-align: start;
		cursor: pointer;
	}

	.summary {
		flex: 0 1 auto;
		margin-left: calc(-1em - var(--space-2));
	}

	.summary:only-child {
		flex: 1;
	}

	.summary:hover,
	button.live:hover {
		color: var(--color-fg);
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
		color: var(--muted-fg);
	}

	span.live,
	.label {
		min-width: 0;
		overflow: hidden;
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

	.name {
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

	.call > :global(button) {
		margin-left: auto;
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
