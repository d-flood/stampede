<script lang="ts">
	import { RequestError } from '#lib/files.svelte.ts';
	import { form } from '#lib/form.ts';
	import { regroup, type Regroupable, type Regroupee } from '#lib/git.ts';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import Eyebrow from '#lib/ui/Eyebrow.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Segmented from '#lib/ui/Segmented.svelte';
	import TextArea from '#lib/ui/TextArea.svelte';
	import Check from '~icons/ph/check';
	import CloudCheck from '~icons/ph/cloud-check';
	import Warning from '~icons/ph/warning';
	import X from '~icons/ph/x';

	type Props = { projectId: string; data: Regroupable; onclose(): void };
	type Group = { key: string; start: number; end: number; color: string; unchanged: boolean };

	let { projectId, data, onclose }: Props = $props();

	let joined = $state.raw(new Set<number>());
	let messages = $state<Record<string, string>>({});
	let anchor = $state<number>();
	let range = $state<[number, number]>();
	let view = $state<'now' | 'after'>('now');
	let heights = $state<Record<string, number>>({});
	let cards = $state<Record<string, number>>({});
	let saving = $state(false);
	let refusal = $state<string>();
	let pushed = $state(false);

	const phone = $derived(form() === 'phone');
	const commits = $derived(data.commits);
	const last = $derived(commits.length - 1);
	const trunk = $derived(data.trunk ?? 'the trunk');
	const groups = $derived.by(() => {
		const found: Group[] = [];
		let start = 0;
		for (let index = 0; index <= last; index++) {
			if (index < last && joined.has(index)) continue;
			found.push({ key: `${start}-${index}`, start, end: index, color: 'var(--border-strong)', unchanged: false });
			start = index + 1;
		}
		let unchanged = true;
		for (const group of found.toReversed()) {
			unchanged = group.unchanged = unchanged && group.start === group.end;
			if (group.start !== group.end) group.color = `var(--subagent-${(group.start % 6) + 1})`;
		}
		return found;
	});
	const changed = $derived(groups.some((group) => !group.unchanged));
	const rewritten = $derived(groups.filter((group) => !group.unchanged).flatMap((group) => commits.slice(group.start, group.end + 1)).filter((commit) => commit.pushed).length);
	const selected = $derived(range ?? (anchor === undefined ? undefined : ([anchor, anchor] as const)));
	const blocked = $derived.by(() => {
		if (!range) return;
		const [from, to] = range;
		for (const { merge } of commits.slice(from, to + 1)) {
			if (merge?.kind !== 'sync') continue;
			if (to !== last) return 'A run with a sync merge must reach the oldest commit';
			if (!merge.foldable) return "Regroup from the branch's first commit to squash its sync merge";
		}
		if (groups.some((group) => group.start === from && group.end === to)) return 'Already one commit';
	});

	function pick(index: number, extend: boolean) {
		if (anchor !== undefined && index !== anchor && (extend || (phone && !range))) range = [Math.min(anchor, index), Math.max(anchor, index)];
		else if (anchor === index && !range) anchor = undefined;
		else (anchor = index), (range = undefined);
	}

	function clear() {
		anchor = range = undefined;
	}

	function squash([from, to]: readonly [number, number]) {
		const next = new Set(joined);
		for (let index = from; index < to; index++) next.add(index);
		next.delete(from - 1);
		next.delete(to);
		joined = next;
		clear();
	}

	function split(group: Group) {
		const next = new Set(joined);
		for (let index = group.start; index < group.end; index++) next.delete(index);
		joined = next;
	}

	function message(group: Group) {
		if (messages[group.key] !== undefined) return messages[group.key];
		const run = commits.slice(group.start, group.end + 1).toReversed();
		const own = run.filter((commit) => !commit.merge && !/^(fixup|amend)! /.test(commit.message));
		return (own.length ? own : run).map((commit) => commit.message).join('\n');
	}

	function ribbon(key: string) {
		const height = heights[key] ?? 0;
		const card = Math.min(cards[key] ?? 0, height);
		const top = (height - card) / 2;
		return `M0 0C24 0 24 ${top} 48 ${top}L48 ${top + card}C24 ${top + card} 24 ${height} 0 ${height}Z`;
	}

	async function save() {
		saving = true;
		refusal = undefined;
		try {
			const chosen = groups.map((group) => ({ newest: commits[group.start].sha, message: group.start === group.end ? undefined : message(group) }));
			await regroup(projectId, data.head, commits[last].sha, chosen, pushed);
			onclose();
		} catch (error) {
			refusal = error instanceof Error ? error.message : String(error);
			pushed = error instanceof RequestError && error.status === 428;
		} finally {
			saving = false;
		}
	}
</script>

{#snippet unit(commit: Regroupee, index: number, alone: boolean)}
	{@const picked = !!selected && index >= selected[0] && index <= selected[1]}
	<button class="unit" class:picked class:top={picked && index === selected?.[0]} class:bottom={picked && index === selected?.[1]} aria-pressed={picked} onmousedown={(event) => event.shiftKey && event.preventDefault()} onclick={(event) => pick(index, event.shiftKey)}>
		<span class={['row', commit.merge?.kind, { trunk: commit.merge?.kind === 'sync' && alone }]}>
			<span class="lane"><span class="dot"></span></span>
			<span class="subject">{commit.subject}</span>
			{#if commit.pushed}<span class="pushed" title="On a remote"><CloudCheck aria-label="On a remote" /></span>{/if}
			{#if !phone}<span class="meta"><span class="sha">{commit.short}</span> {commit.date}</span>{/if}
		</span>
		{#if commit.merge?.kind === 'side'}
			{@const { commits: side, more } = commit.merge}
			{#each side as brought, offset (brought.sha)}
				<span class="row brought" class:end={offset === side.length - 1 && !more}>
					<span class="lane"><span class="dot"></span></span>
					<span class="subject">{brought.subject}</span>
					{#if !phone}<span class="meta"><span class="sha">{brought.short}</span> {brought.author}</span>{/if}
				</span>
			{/each}
			{#if more}<span class="row brought end"><span class="lane"></span><span class="subject">{more} more</span></span>{/if}
		{:else if commit.merge?.kind === 'sync'}
			<span class="row upstream"><span class="lane"><span class="dot"></span></span><span class="subject">{commit.merge.count} commit{commit.merge.count === 1 ? '' : 's'} from {trunk}</span></span>
		{/if}
	</button>
	{#if commit.merge?.kind === 'sync' && alone}
		<div class="kept">
			<span>Commits below were written against an older {trunk}, so this merge stays unless they're squashed with it.</span>
			{#if commit.merge.foldable && index < last}<Button size="sm" variant="ghost" onclick={() => squash([index, last])}>Squash Everything Below</Button>{/if}
		</div>
	{/if}
{/snippet}

{#snippet card(group: Group)}
	{#if group.start === group.end}
		{@const commit = commits[group.start]}
		<div class="card single" bind:clientHeight={cards[group.key]}>
			<span class="subject">{commit.subject}</span>
			<span class="meta">{group.unchanged ? commit.short : 'rewritten'}</span>
		</div>
	{:else}
		{@const folded = commits.slice(group.start, group.end + 1).find((commit) => commit.merge?.kind === 'sync')?.merge}
		<div class="card" bind:clientHeight={cards[group.key]}>
			<TextArea aria-label="Message" spellcheck="true" bind:value={() => message(group), (value) => (messages[group.key] = value)} />
			<div class="facts">
				<span>{group.end - group.start + 1} commits{folded?.kind === 'sync' ? ` · onto ${trunk} ${folded.onto}` : ''}</span>
				<Button size="sm" variant="ghost" onclick={() => split(group)}>Split Apart</Button>
			</div>
		</div>
	{/if}
{/snippet}

{#snippet status()}
	<div class="status" role="status">
		{#if refusal}
			<span class="warn" role="alert">{refusal}</span>
		{:else if selected && range}
			<span>{range[1] - range[0] + 1} commits</span>
			{#if blocked}<span class="warn">{blocked}</span>{/if}
			<Button size="sm" variant="primary" disabled={!!blocked} onclick={() => squash(range!)}>Squash Into One</Button>
			<IconButton label="Clear selection" onclick={clear}><X /></IconButton>
		{:else if selected}
			<span>{phone ? 'Tap another commit to choose a run' : 'Shift-click another commit to choose a run'}</span>
			<IconButton label="Clear selection" onclick={clear}><X /></IconButton>
		{:else if changed}
			<span class="ok"><Check aria-hidden="true" />Same final tree</span>
			{#if rewritten}<span class="warn"><Warning aria-hidden="true" />Rewrites {rewritten} pushed</span>{/if}
		{:else}
			<span>{phone ? 'Tap two commits to choose a run to squash' : 'Click a commit, then shift-click another to choose a run to squash'}</span>
		{/if}
	</div>
{/snippet}

<Dialog bind:open={() => true, (open) => !open && onclose()} wide full={phone} title="Regroup commits">
	{#if phone}
		<div class="views">
			<Segmented
				options={[
					{ value: 'now', label: `Now · ${commits.length}` },
					{ value: 'after', label: `After · ${groups.length}` }
				]}
				bind:value={view}
			/>
		</div>
	{:else}
		<div class="columns heads">
			<span><Eyebrow>Now</Eyebrow>{commits.length} commits</span>
			<span></span>
			<span><Eyebrow>After</Eyebrow>{groups.length} commit{groups.length === 1 ? '' : 's'}</span>
		</div>
	{/if}
	<div class="body" class:phone>
		{#each groups as group (group.key)}
			<div class="columns group" class:grouped={group.start !== group.end} style:--c={group.color} bind:clientHeight={heights[group.key]}>
				{#if !phone || view === 'now'}
					<div class="graph">
						{#each commits.slice(group.start, group.end + 1) as commit, offset (commit.sha)}
							{@render unit(commit, group.start + offset, group.start === group.end)}
						{/each}
					</div>
				{/if}
				{#if !phone}
					<svg class="ribbon" viewBox="0 0 48 {Math.max(heights[group.key] ?? 0, 1)}" preserveAspectRatio="none" aria-hidden="true"><path d={ribbon(group.key)} /></svg>
				{/if}
				{#if !phone || view === 'after'}{@render card(group)}{/if}
			</div>
		{/each}
	</div>
	{#if phone}{@render status()}{/if}
	{#snippet actions()}
		{#if !phone}{@render status()}{/if}
		<Button disabled={saving} onclick={onclose}>Cancel</Button>
		<Button variant={pushed ? 'danger' : 'primary'} disabled={!changed || saving} onclick={save}>{saving ? 'Regrouping…' : pushed ? 'Regroup Anyway' : 'Regroup'}</Button>
	{/snippet}
</Dialog>

<style>
	.columns {
		display: grid;
		grid-template-columns: minmax(0, 1.15fr) var(--tile) minmax(0, 0.85fr);
	}

	.heads {
		flex: none;
		padding: var(--space-2) var(--space-6);
		border-bottom: var(--border-width) solid var(--border);
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.heads > span {
		display: flex;
		align-items: baseline;
		gap: var(--space-2);
	}

	.views {
		flex: none;
		padding: var(--space-2) var(--space-4);
		border-bottom: var(--border-width) solid var(--border);
	}

	.body {
		--x0: 0.75rem;
		--x1: 1.75rem;
		flex: 1;
		min-height: 0;
		padding: var(--space-3) var(--space-6) var(--space-6);
		overflow-y: auto;
	}

	.body.phone {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		padding: var(--space-3) var(--space-4) var(--space-6);
	}

	.body.phone .group {
		display: block;
	}

	.group {
		align-items: center;
		transition: opacity var(--dur-fast);
	}

	.body:has(.card:hover) .group:not(:has(.card:hover)) {
		opacity: 0.45;
	}

	.graph {
		position: relative;
		align-self: stretch;
		user-select: none;
		background: linear-gradient(var(--border-strong), var(--border-strong)) no-repeat calc(var(--x0) - var(--border-width)) 0 / calc(2 * var(--border-width)) 100%;
	}

	.body:not(.phone) .group:first-child .graph {
		background-position-y: calc(var(--target-min) / 2);
	}

	.grouped .graph {
		background-color: color-mix(in oklab, var(--c) 10%, transparent);
	}

	.grouped .graph::before {
		position: absolute;
		inset: 0 auto 0 0;
		width: 3px;
		background: var(--c);
		content: '';
	}

	.phone .graph {
		border-radius: var(--radius-md);
	}

	.unit {
		position: relative;
		display: block;
		width: 100%;
		padding: 0;
		border: none;
		background: none;
		color: inherit;
		font: inherit;
		text-align: left;
		cursor: pointer;
	}

	.unit:hover {
		background: var(--hover);
	}

	.unit:focus-visible {
		outline: var(--focus-ring);
		outline-offset: var(--focus-inset);
	}

	.picked::after {
		position: absolute;
		inset: 0;
		border-inline: calc(2 * var(--border-width)) solid var(--selected-border);
		background: var(--selected-bg);
		pointer-events: none;
		content: '';
	}

	.picked.top::after {
		border-top: calc(2 * var(--border-width)) solid var(--selected-border);
		border-start-start-radius: var(--radius-sm);
		border-start-end-radius: var(--radius-sm);
	}

	.picked.bottom::after {
		border-bottom: calc(2 * var(--border-width)) solid var(--selected-border);
		border-end-start-radius: var(--radius-sm);
		border-end-end-radius: var(--radius-sm);
	}

	.row {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-height: var(--target-min);
		padding-right: var(--space-2);
	}

	.lane {
		position: relative;
		flex: none;
		align-self: stretch;
		width: calc(var(--x1) + var(--space-3));
	}

	.dot {
		position: absolute;
		z-index: 1;
		top: 50%;
		left: var(--x0);
		box-sizing: border-box;
		width: 9px;
		height: 9px;
		border: calc(2 * var(--border-width)) solid var(--muted-fg);
		border-radius: 50%;
		background: var(--color-bg);
		translate: -50% -50%;
	}

	.side .dot {
		border-color: var(--ansi-5);
	}

	.sync .dot {
		border-color: var(--ansi-6);
	}

	.side .lane::after,
	.sync .lane::after {
		position: absolute;
		top: calc(50% - var(--border-width));
		bottom: 0;
		box-sizing: border-box;
		left: var(--x0);
		width: calc(var(--x1) - var(--x0) + var(--border-width));
		border-top: calc(2 * var(--border-width)) solid var(--ansi-5);
		border-right: calc(2 * var(--border-width)) solid var(--ansi-5);
		border-top-right-radius: var(--radius-md);
		content: '';
	}

	.sync .lane::after {
		border-color: var(--ansi-6);
	}

	.brought,
	.upstream {
		min-height: calc(var(--target-min) - var(--space-1));
		color: var(--muted-fg);
	}

	.brought .lane::before,
	.upstream .lane::before {
		position: absolute;
		top: 0;
		bottom: 0;
		left: calc(var(--x1) - var(--border-width));
		box-sizing: border-box;
		border-left: calc(2 * var(--border-width)) solid var(--ansi-5);
		content: '';
	}

	.upstream .lane::before {
		border-left-style: dashed;
		border-color: var(--ansi-6);
	}

	.brought .dot,
	.upstream .dot {
		left: var(--x1);
		width: 7px;
		height: 7px;
		border-color: var(--ansi-5);
	}

	.upstream .dot {
		border-color: var(--ansi-6);
	}

	.brought.end .lane::before {
		bottom: 50%;
	}

	.brought.end .lane::after {
		position: absolute;
		top: 50%;
		bottom: 0;
		box-sizing: border-box;
		left: var(--x0);
		width: calc(var(--x1) - var(--x0) + var(--border-width));
		border-right: calc(2 * var(--border-width)) solid var(--ansi-5);
		border-bottom: calc(2 * var(--border-width)) solid var(--ansi-5);
		border-bottom-right-radius: var(--radius-md);
		content: '';
	}

	.trunk .subject {
		font-weight: 500;
	}

	.subject {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.meta,
	.pushed {
		flex: none;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		white-space: nowrap;
	}

	.pushed {
		display: inline-flex;
	}

	.sha {
		font-family: var(--font-mono);
	}

	.kept {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1) var(--space-2);
		padding: 0 var(--space-2) var(--space-2) calc(var(--x1) + var(--space-3) + var(--space-2));
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.ribbon {
		align-self: stretch;
		width: 100%;
		height: 100%;
	}

	.ribbon path {
		fill: color-mix(in oklab, var(--c) 14%, transparent);
		stroke: color-mix(in oklab, var(--c) 40%, transparent);
		stroke-width: var(--border-width);
		vector-effect: non-scaling-stroke;
	}

	.group:not(.grouped) .ribbon path {
		fill: color-mix(in oklab, var(--c) 8%, transparent);
	}

	.card {
		position: relative;
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		box-sizing: border-box;
		min-width: 0;
		margin-block: var(--space-1);
		padding: var(--space-2) var(--space-3) var(--space-2) var(--space-4);
		border: var(--border-width) solid color-mix(in oklab, var(--c) 45%, var(--border));
		border-radius: var(--radius-lg);
		background: var(--card);
	}

	.card::before {
		position: absolute;
		inset: var(--space-2) auto var(--space-2) var(--space-1);
		width: 3px;
		border-radius: var(--radius-pill);
		background: var(--c);
		content: '';
	}

	.card :global(textarea) {
		max-height: 10lh;
		field-sizing: content;
	}

	.card.single {
		flex-direction: row;
		align-items: center;
		padding-block: var(--space-1);
		border-color: var(--border);
	}

	.card.single::before {
		display: none;
	}

	.facts {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.status {
		display: flex;
		flex: 1;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1) var(--space-3);
		min-width: 0;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.phone + .status {
		flex: none;
		padding: var(--space-2) var(--space-4);
		border-top: var(--border-width) solid var(--border);
	}

	.ok,
	.warn {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
	}

	.ok {
		color: var(--git-added);
	}

	.warn {
		color: var(--git-deleted);
	}
</style>
