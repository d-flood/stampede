<script lang="ts">
	import Text from '#lib/chat/Text.svelte';
	import type { Epic } from '#lib/device.svelte.ts';
	import Progress from '#lib/epics/Progress.svelte';
	import TicketDetail from '#lib/epics/Ticket.svelte';
	import { counts, label, resolved, statuses, tones, type OpenEpic } from '#lib/epics.svelte.ts';
	import { form } from '#lib/form.ts';
	import { painted } from '#lib/paint.svelte.ts';
	import IconButton from '#lib/ui/IconButton.svelte';
	import { onMount } from 'svelte';
	import CaretLeft from '~icons/ph/caret-left';
	import CaretRight from '~icons/ph/caret-right';

	type Props = { open: OpenEpic; epic?: Epic; loaded: boolean; active: boolean };

	let { open, epic, loaded, active }: Props = $props();

	const phone = $derived(form() === 'phone');
	const tickets = $derived(epic?.tickets ?? []);
	const ticket = $derived(tickets.find((ticket) => ticket.number === open.ticket));
	const folded = $derived(phone ? tickets.filter((ticket) => resolved(ticket.status)).length : 0);
	const shown = $derived(folded && !open.showResolved ? tickets.filter((ticket) => !resolved(ticket.status)) : tickets);
	const lines = $derived(epic?.brief.trimEnd().split('\n').length ?? 0);

	onMount(() => painted(open.id));
</script>

{#snippet back(title: string, onclick: () => void)}
	<header class="screen">
		<IconButton label="Back to {open.slug}" {onclick}><CaretLeft /></IconButton>
		<h2>{title}</h2>
	</header>
{/snippet}

{#snippet brief(epic: Epic)}
	<Text item={{ kind: 'text', text: epic.brief, streaming: false, phase: 'final' }} />
{/snippet}

{#snippet setup(epic: Epic)}
	{#if epic.setup === null}
		<p class="unset">Not set yet</p>
	{:else if epic.setup === ''}
		<p class="unset">Nothing to run</p>
	{:else}
		<code>{epic.setup}</code>
	{/if}
{/snippet}

<div class="epic" class:phone role="tabpanel" aria-label={open.name} hidden={!active}>
	{#if !epic}
		{#if loaded}<p class="missing">There is no Epic {open.slug} in this Project.</p>{/if}
	{:else if open.page === 'brief'}
		{@render back('Brief', () => (open.page = undefined))}
		<div class="scroll">{@render brief(epic)}</div>
	{:else if open.page === 'setup'}
		{@render back('Setup Command', () => (open.page = undefined))}
		<div class="scroll">{@render setup(epic)}</div>
	{:else if phone && ticket}
		{@render back(`${epic.slug}/${ticket.number}`, () => (open.ticket = undefined))}
		<div class="scroll"><TicketDetail {ticket} /></div>
	{:else}
		{@const tally = counts(epic)}
		<div class="scroll">
			<header class="head">
				<div class="name">
					<h1>{epic.slug}</h1>
					<span>{epic.title}</span>
				</div>
				<div class="progress">
					<Progress {epic} />
					<ul class="legend">
						{#each statuses as status (status)}
							{#if tally[status]}
								<li><span class="count" style:--tone={tones[status]}>{tally[status]}</span> {label(status)}</li>
							{/if}
						{/each}
					</ul>
				</div>
			</header>
			{#if phone}
				<div class="pages">
					<button onclick={() => (open.page = 'brief')}>Brief <CaretRight /></button>
					<button onclick={() => (open.page = 'setup')}>Setup <CaretRight /></button>
				</div>
			{/if}
			<div class="body">
				<section aria-label="Tickets" class="tickets">
					{#if !phone}
						<div class="row columns" aria-hidden="true">
							<span>#</span><span>Ticket</span><span>Blockers</span><span>Status</span><span>Held by</span>
						</div>
					{/if}
					{#each shown as ticket (ticket.number)}
						{@const expanded = !phone && open.ticket === ticket.number}
						<button
							class="row"
							class:resolved={resolved(ticket.status)}
							class:expanded
							aria-expanded={phone ? undefined : expanded}
							onclick={() => (open.ticket = expanded ? undefined : ticket.number)}
						>
							<span class="number">{ticket.number}</span>
							{#if phone}
								<span class="text">
									<span class="title">{ticket.title}</span>
									<span class="sub" style:--tone={tones[ticket.status]}>
										{label(ticket.status)}{#if ticket.blockers.length}{' '}· after {ticket.blockers.join(', ')}{/if}
									</span>
								</span>
								<CaretRight />
							{:else}
								<span class="title">{ticket.title}</span>
								<span class="number">{ticket.blockers.join(', ')}</span>
								<span class="status" style:--tone={tones[ticket.status]}>{label(ticket.status)}</span>
								<span></span>
							{/if}
						</button>
						{#if expanded}
							<div class="detail"><TicketDetail {ticket} /></div>
						{/if}
					{:else}
						<p class="unset">No Tickets yet.</p>
					{/each}
					{#if folded}
						<button class="fold" onclick={() => (open.showResolved = !open.showResolved)}>
							{open.showResolved ? 'Hide' : 'Show'} {folded} resolved
						</button>
					{/if}
				</section>
				{#if !phone}
					<aside>
						<section aria-label="Brief">
							<h2>Brief</h2>
							<div class="preview">{@render brief(epic)}</div>
							<button class="link" onclick={() => (open.page = 'brief')}>Read all {lines} lines</button>
						</section>
						<section aria-label="Setup Command">
							<h2>Setup Command</h2>
							{@render setup(epic)}
						</section>
					</aside>
				{/if}
			</div>
		</div>
	{/if}
</div>

<style>
	.epic {
		display: flex;
		flex-direction: column;
		height: 100%;
		min-height: 0;
	}

	.epic[hidden] {
		display: none;
	}

	.scroll {
		flex: 1;
		min-height: 0;
		overflow: auto;
		padding: var(--space-4) var(--space-6) var(--space-6);
	}

	.phone .scroll {
		padding: var(--space-3);
	}

	.missing,
	.unset {
		margin: 0;
		color: var(--muted-fg);
	}

	.missing {
		padding: var(--space-6);
	}

	header.screen {
		display: flex;
		flex: none;
		align-items: center;
		gap: var(--space-1);
		padding: var(--space-1);
		border-bottom: var(--border-width) solid var(--border);
	}

	header.screen h2 {
		margin: 0;
		font-family: var(--font-mono);
		font-size: var(--font-size-md);
	}

	.head {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-end;
		justify-content: space-between;
		gap: var(--space-3) var(--space-6);
		margin-bottom: var(--space-4);
	}

	.name {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}

	.name span {
		color: var(--muted-fg);
	}

	h1 {
		margin: 0;
		font-size: 1.5em;
	}

	.phone h1 {
		font-size: 1.25em;
	}

	.progress {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		width: 22rem;
		max-width: 100%;
	}

	.phone .progress {
		width: 100%;
	}

	.legend {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1) var(--space-4);
		margin: 0;
		padding: 0;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		list-style: none;
	}

	.count {
		color: var(--tone);
		font-variant-numeric: tabular-nums;
	}

	.pages {
		display: flex;
		gap: var(--space-2);
		margin-bottom: var(--space-3);
	}

	.pages button {
		display: flex;
		flex: 1;
		align-items: center;
		justify-content: space-between;
		min-height: var(--target-min);
		padding: 0 var(--space-4);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
		background: none;
		color: var(--color-fg);
		font: inherit;
	}

	.pages :global(svg),
	.phone .row :global(svg) {
		flex: none;
		color: var(--muted-fg);
	}

	.body {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-start;
		gap: var(--space-6);
	}

	.tickets {
		display: flex;
		flex: 1 1 32rem;
		flex-direction: column;
		min-width: 0;
		overflow: hidden;
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
	}

	.phone .tickets {
		gap: var(--space-2);
		overflow: visible;
		border: none;
	}

	.tickets > .unset {
		padding: var(--space-3) var(--space-4);
	}

	.row {
		display: grid;
		grid-template-columns: 2.5rem minmax(0, 1fr) 6rem 7rem 8rem;
		gap: var(--space-3);
		align-items: baseline;
		padding: var(--space-2) var(--space-4);
		border: none;
		border-left: 2px solid transparent;
		background: none;
		color: var(--color-fg);
		font: inherit;
		text-align: left;
	}

	button.row {
		cursor: pointer;
	}

	button.row:hover {
		background: var(--hover);
	}

	button.row:focus-visible,
	.pages button:focus-visible,
	.fold:focus-visible,
	.link:focus-visible {
		outline: var(--border-width) solid var(--status-working);
		outline-offset: calc(-1 * var(--border-width));
	}

	.columns {
		border-bottom: var(--border-width) solid var(--border);
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.resolved {
		color: var(--muted-fg);
	}

	.expanded {
		border-left-color: var(--color-fg);
		background: var(--surface-1);
	}

	.number {
		color: var(--muted-fg);
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
	}

	.row .title {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.expanded .title {
		font-weight: 600;
	}

	.status,
	.sub {
		color: var(--tone);
	}

	.detail {
		padding: var(--space-3) var(--space-4) var(--space-4) calc(2.5rem + var(--space-4) + var(--space-3));
		border-left: 2px solid var(--color-fg);
		background: var(--surface-1);
	}

	.phone .row {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		min-height: calc(var(--target-min) + var(--space-2));
		padding: var(--space-2) var(--space-3);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
	}

	.phone .number {
		flex: none;
		width: 1.75em;
	}

	.text {
		display: flex;
		flex: 1;
		flex-direction: column;
		min-width: 0;
	}

	.sub {
		font-size: var(--font-size-sm);
	}

	.fold {
		min-height: var(--target-min);
		border: none;
		background: none;
		color: var(--muted-fg);
		font: inherit;
		cursor: pointer;
	}

	aside {
		display: flex;
		flex: 0 1 20rem;
		flex-direction: column;
		gap: var(--space-4);
		min-width: 16rem;
	}

	aside section {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		padding: var(--space-4);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
	}

	aside h2 {
		margin: 0;
		font-size: var(--font-size-md);
	}

	.preview {
		max-height: 10em;
		overflow: hidden;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.link {
		align-self: flex-start;
		padding: 0;
		border: none;
		background: none;
		color: var(--status-working);
		font: inherit;
		font-size: var(--font-size-sm);
		cursor: pointer;
	}

	code {
		padding: var(--space-2) var(--space-3);
		border-radius: var(--radius-md);
		background: var(--surface-2);
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		overflow-wrap: anywhere;
	}

	.phone code {
		display: block;
		font-size: var(--font-size-md);
	}
</style>
