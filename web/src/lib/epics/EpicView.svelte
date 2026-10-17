<script lang="ts">
	import Text from '#lib/chat/Text.svelte';
	import { rigs } from '#lib/config.svelte.ts';
	import type { Chat, Device, Epic, Ticket } from '#lib/device.svelte.ts';
	import Blockers from '#lib/epics/Blockers.svelte';
	import Progress from '#lib/epics/Progress.svelte';
	import TicketDetail from '#lib/epics/Ticket.svelte';
	import { counts, label, record, resolved, statuses, toggle, tones, type Edit, type OpenEpic } from '#lib/epics.svelte.ts';
	import { form } from '#lib/form.ts';
	import { painted } from '#lib/paint.svelte.ts';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import { onMount } from 'svelte';
	import CaretDown from '~icons/ph/caret-down';
	import CaretLeft from '~icons/ph/caret-left';
	import CaretRight from '~icons/ph/caret-right';
	import Minus from '~icons/ph/minus';
	import Plus from '~icons/ph/plus';

	type Props = {
		open: OpenEpic;
		epic?: Epic;
		loaded: boolean;
		active: boolean;
		device: Device;
		chats: Chat[];
		onchat: (chatId: string) => void;
	};

	let { open, epic, loaded, active, device, chats, onchat }: Props = $props();

	const phone = $derived(form() === 'phone');
	const tickets = $derived(epic?.tickets ?? []);
	const ticket = $derived(tickets.find((ticket) => ticket.number === open.ticket));
	const folded = $derived(phone ? tickets.filter((ticket) => resolved(ticket.status)).length : 0);
	const shown = $derived(folded && !open.showResolved ? tickets.filter((ticket) => !resolved(ticket.status)) : tickets);
	const lines = $derived(epic?.brief.trimEnd().split('\n').length ?? 0);
	const next = $derived(Math.max(0, ...tickets.map((ticket) => ticket.number)) + 1);
	const choices = $derived(rigs());
	const rig = $derived(choices.some((choice) => choice.name === open.rig) ? open.rig : choices[0]?.name);

	let draft = $state('');
	let title = $state('');
	let blockers = $state<number[]>([]);
	let setup = $state<string>();
	let error = $state<string>();
	let saving = $state(false);
	let archiving = $state(false);
	let refused = $state<string>();
	let returnTo: OpenEpic['page'];

	onMount(() => painted(open.id));

	function edit(page: 'edit-brief' | 'edit-ticket' | 'new-ticket', text: string) {
		draft = text;
		title = '';
		blockers = [];
		error = undefined;
		returnTo = open.page;
		open.page = page;
	}

	function editSetup(command: string | null) {
		error = undefined;
		setup = command ?? '';
	}

	async function save(edit: Edit, done: () => void) {
		if (saving || !epic) return;
		saving = true;
		error = await record(epic.slug, edit);
		saving = false;
		if (!error) done();
	}

	function saveDraft() {
		if (!draft.trim()) return;
		const back = () => (open.page = returnTo);
		if (open.page === 'edit-brief') save({ kind: 'brief.set', brief: draft }, back);
		else if (open.page === 'edit-ticket' && ticket) save({ kind: 'ticket.body.set', ticket: ticket.number, body: draft }, back);
		else if (open.page === 'new-ticket' && title.trim()) {
			save({ kind: 'ticket.added', ticket: next, title: title.trim(), body: draft, blockers }, back);
		}
	}

	function saveSetup(event: SubmitEvent) {
		event.preventDefault();
		if (setup !== undefined) save({ kind: 'setup.set', command: setup.trim() }, () => (setup = undefined));
	}

	async function run(edit: Edit) {
		if (epic) refused = await record(epic.slug, edit);
	}

	function chooseRig(name: string) {
		if (epic?.run) run({ kind: 'run.changed', rig: name });
		else open.rig = name;
	}

	function limit(parallel: number) {
		if (parallel < 1) return;
		if (epic?.run) run({ kind: 'run.changed', parallel });
		else open.parallel = parallel;
	}

	function chatOf(ticket: Ticket) {
		return chats.find((chat) => chat.id === ticket.claim?.chat);
	}

	function holder(ticket: Ticket) {
		if (ticket.status !== 'in-progress' || !ticket.claim) return ticket.attention ? 'needs attention' : '';
		return `${ticket.claim.rig} · ${chatOf(ticket)?.status ?? 'closed'}`;
	}
</script>

{#snippet back(title: string, onclick: () => void, onedit?: () => void)}
	<header class="screen">
		<IconButton label="Back to {open.slug}" {onclick}><CaretLeft /></IconButton>
		<h2>{title}</h2>
		{#if onedit}<button class="link" onclick={onedit}>Edit</button>{/if}
	</header>
{/snippet}

{#snippet editing(title: string, action: string, ready: boolean)}
	<header class="screen editing">
		<h2>{title}</h2>
		<Button onclick={() => (open.page = returnTo)}>Cancel</Button>
		<Button variant="primary" disabled={saving || !ready} onclick={saveDraft}>{action}</Button>
	</header>
	{#if error}<p class="error" role="alert">{error}</p>{/if}
{/snippet}

{#snippet markdown(label: string, autofocus = true)}
	{#await import('#lib/epics/Markdown.svelte') then { default: Markdown }}
		<Markdown bind:text={draft} {label} {autofocus} onsave={saveDraft} />
	{/await}
{/snippet}

{#snippet brief(epic: Epic)}
	<Text item={{ kind: 'text', text: epic.brief, streaming: false, phase: 'final' }} />
{/snippet}

{#snippet command(epic: Epic)}
	{#if setup !== undefined}
		<form class="setup" onsubmit={saveSetup}>
			<input bind:value={setup} aria-label="Setup Command" placeholder="pnpm install" spellcheck="false" autocomplete="off" enterkeyhint="done" />
			<p class="unset">Leave it empty when a fresh worktree needs nothing.</p>
			{#if error}<p class="error" role="alert">{error}</p>{/if}
			<div class="buttons">
				<Button type="button" onclick={() => (setup = undefined)}>Cancel</Button>
				<Button type="submit" variant="primary" disabled={saving}>Save</Button>
			</div>
		</form>
	{:else if epic.setup === null}
		<p class="unset">Not set yet</p>
	{:else if epic.setup === ''}
		<p class="unset">Nothing to run</p>
	{:else}
		<code>{epic.setup}</code>
	{/if}
{/snippet}

{#snippet settings(epic: Epic)}
	{@const parallel = epic.run?.parallel ?? open.parallel}
	<label class="setting">
		<span>Rig</span>
		<select value={epic.run?.rig ?? rig} onchange={(event) => chooseRig(event.currentTarget.value)}>
			{#each choices as choice (choice.name)}
				<option value={choice.name}>{choice.name}</option>
			{/each}
		</select>
	</label>
	<span class="setting" role="group" aria-label="Parallel">
		<span aria-hidden="true">Parallel</span>
		<IconButton label="Fewer" disabled={parallel <= 1} onclick={() => limit(parallel - 1)}><Minus /></IconButton>
		<output class="limit">{parallel}</output>
		<IconButton label="More" onclick={() => limit(parallel + 1)}><Plus /></IconButton>
	</span>
{/snippet}

{#snippet runBar(epic: Epic)}
	{@const live = epic.run}
	<section class="run" aria-label="Run">
		{#if live}
			<div class="top">
				<span class="live">Run live</span>
				{#if phone}
					<button class="summary" aria-expanded={open.showRun} onclick={() => (open.showRun = !open.showRun)}>
						{live.rig} · ×{live.parallel}
						<CaretDown />
					</button>
				{/if}
			</div>
		{/if}
		{#if !phone || !live || open.showRun}
			<div class="settings">{@render settings(epic)}</div>
		{/if}
		{#if !live}
			<Button variant="primary" disabled={!rig || epic.setup === null} onclick={() => rig && run({ kind: 'run.started', rig, parallel: open.parallel })}>
				Start Run
			</Button>
			{#if epic.setup === null}<p class="note">Record a Setup Command before starting a Run.</p>{/if}
		{/if}
		{#if refused}<p class="note error" role="alert">{refused}</p>{/if}
	</section>
{/snippet}

{#snippet archive()}
	<div class="archive"><Button onclick={() => ((error = undefined), (archiving = true))}>Archive Epic</Button></div>
{/snippet}

<div class="epic" class:phone role="tabpanel" aria-label={open.name} hidden={!active}>
	{#if !epic}
		{#if loaded}<p class="missing">There is no Epic {open.slug} in this Project.</p>{/if}
	{:else if open.page === 'edit-brief'}
		{@render editing('Brief', 'Save', !!draft.trim())}
		{@render markdown('Brief')}
	{:else if open.page === 'edit-ticket' && ticket}
		{@render editing(`${epic.slug}/${ticket.number}`, 'Save', !!draft.trim())}
		{@render markdown('Body')}
	{:else if open.page === 'new-ticket'}
		{@render editing(`${epic.slug}/${next}`, 'Add', !!draft.trim() && !!title.trim())}
		<div class="fields">
			<input bind:value={title} aria-label="Title" placeholder="Title" autocomplete="off" />
			{#if tickets.length}
				<div class="field">
					<span>Blocked by</span>
					<Blockers {tickets} {blockers} ontoggle={(number) => (blockers = toggle(blockers, number))} />
				</div>
			{/if}
		</div>
		{@render markdown('Body', false)}
	{:else if open.page === 'brief'}
		{@render back('Brief', () => (open.page = undefined), () => edit('edit-brief', epic.brief))}
		<div class="scroll">{@render brief(epic)}</div>
	{:else if open.page === 'setup'}
		{@render back('Setup Command', () => ((open.page = undefined), (setup = undefined)), setup === undefined ? () => editSetup(epic.setup) : undefined)}
		<div class="scroll">{@render command(epic)}</div>
	{:else if phone && ticket}
		{@render back(`${epic.slug}/${ticket.number}`, () => (open.ticket = undefined), () => edit('edit-ticket', ticket.body))}
		<div class="scroll">
			<TicketDetail {epic} {ticket} onedit={() => edit('edit-ticket', ticket.body)} {open} chat={chatOf(ticket)} {device} {onchat} />
		</div>
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
			{@render runBar(epic)}
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
										{label(ticket.status)}{#if ticket.blockers.length}{' '}· after {ticket.blockers.join(', ')}{/if}{#if holder(ticket)}{' '}· {holder(ticket)}{/if}{#if ticket.claim?.edited}{' '}· edited{/if}
									</span>
								</span>
								<CaretRight />
							{:else}
								<span class="title">{ticket.title}</span>
								<span class="number">{ticket.blockers.join(', ')}</span>
								<span class="status" style:--tone={tones[ticket.status]}>{label(ticket.status)}</span>
								<span class="held" class:flagged={!!ticket.attention}>{holder(ticket)}</span>
							{/if}
						</button>
						{#if expanded}
							<div class="detail">
								<TicketDetail {epic} {ticket} onedit={() => edit('edit-ticket', ticket.body)} {open} chat={chatOf(ticket)} {device} {onchat} />
							</div>
						{/if}
					{:else}
						<p class="unset">No Tickets yet.</p>
					{/each}
					{#if folded}
						<button class="fold" onclick={() => (open.showResolved = !open.showResolved)}>
							{open.showResolved ? 'Hide' : 'Show'} {folded} resolved
						</button>
					{/if}
					<button class="add" onclick={() => edit('new-ticket', '')}><Plus /> Add a Ticket</button>
				</section>
				{#if phone}
					{@render archive()}
				{:else}
					<aside>
						<section aria-label="Brief">
							<div class="heading">
								<h2>Brief</h2>
								<button class="link" onclick={() => edit('edit-brief', epic.brief)}>Edit</button>
							</div>
							<div class="preview">{@render brief(epic)}</div>
							<button class="link" onclick={() => (open.page = 'brief')}>Read all {lines} lines</button>
						</section>
						<section aria-label="Setup Command">
							<div class="heading">
								<h2>Setup Command</h2>
								{#if setup === undefined}<button class="link" onclick={() => editSetup(epic.setup)}>Edit</button>{/if}
							</div>
							{@render command(epic)}
						</section>
						{@render archive()}
					</aside>
				{/if}
			</div>
		</div>
	{/if}
</div>

<Dialog
	bind:open={archiving}
	title="Archive {open.slug}?"
	description="It disappears from every list but is not deleted. Nothing in Oikonomos unarchives it yet."
>
	{#if error}<p class="error" role="alert">{error}</p>{/if}
	{#snippet actions()}
		<Button onclick={() => (archiving = false)}>Cancel</Button>
		<Button variant="primary" disabled={saving} onclick={() => save({ kind: 'epic.archived' }, () => (archiving = false))}>Archive</Button>
	{/snippet}
</Dialog>

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
		flex: 1;
		min-width: 0;
		margin: 0;
		overflow: hidden;
		font-family: var(--font-mono);
		font-size: var(--font-size-md);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	header.editing {
		gap: var(--space-2);
		padding-left: var(--space-3);
	}

	header.screen .link {
		align-self: stretch;
		padding: 0 var(--space-3);
		font-size: var(--font-size-md);
	}

	.error {
		margin: 0;
		color: var(--ansi-1);
		font-size: var(--font-size-sm);
	}

	header + .error {
		padding: var(--space-2) var(--space-3);
		border-bottom: var(--border-width) solid var(--border);
	}

	.fields {
		display: flex;
		flex: none;
		flex-direction: column;
		gap: var(--space-3);
		padding: var(--space-3);
		border-bottom: var(--border-width) solid var(--border);
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	input {
		box-sizing: border-box;
		width: 100%;
		min-height: var(--target-min);
		padding: 0 var(--space-3);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-1);
		color: var(--color-fg);
		font: inherit;
		outline: none;
	}

	input:focus-visible {
		border-color: var(--status-working);
	}

	.setup input {
		font-family: var(--font-mono);
	}

	.setup {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}

	.buttons {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-2);
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

	.run {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-4);
		margin-bottom: var(--space-4);
		padding: var(--space-2) var(--space-4);
		border-radius: var(--radius-lg);
		background: var(--surface-2);
	}

	.top {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
	}

	.live {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		font-weight: 600;
	}

	.live::before {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--status-done);
		content: '';
	}

	.settings {
		display: flex;
		flex: 1;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-4);
	}

	.setting {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		color: var(--muted-fg);
	}

	select {
		min-height: var(--target-min);
		padding: 0 var(--space-2);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-1);
		color: var(--color-fg);
		font: inherit;
	}

	.limit {
		min-width: 1.5em;
		color: var(--color-fg);
		font-family: var(--font-mono);
		text-align: center;
	}

	.run .note {
		flex-basis: 100%;
		margin: 0;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.run .error {
		color: var(--ansi-1);
	}

	.summary {
		display: flex;
		align-items: center;
		gap: var(--space-1);
		min-height: var(--target-min);
		padding: 0;
		border: none;
		background: none;
		color: var(--muted-fg);
		font: inherit;
		font-size: var(--font-size-sm);
	}

	.phone .run {
		flex-direction: column;
		align-items: stretch;
		margin-bottom: var(--space-3);
		padding: var(--space-3);
	}

	.phone .settings {
		flex-direction: column;
		align-items: stretch;
	}

	.phone .setting select {
		flex: 1;
	}

	.phone [role='group'] > span:first-child {
		flex: 1;
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

	.held {
		overflow: hidden;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.flagged {
		color: var(--status-waiting);
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

	.fold,
	.add {
		min-height: var(--target-min);
		border: none;
		background: none;
		color: var(--muted-fg);
		font: inherit;
		cursor: pointer;
	}

	.add {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-2) var(--space-4);
		border-top: var(--border-width) solid var(--border);
	}

	.add:hover {
		background: var(--hover);
		color: var(--color-fg);
	}

	.phone .add {
		justify-content: center;
		border: var(--border-width) dashed var(--border);
		border-radius: var(--radius-lg);
	}

	.add:focus-visible {
		outline: var(--border-width) solid var(--status-working);
		outline-offset: calc(-1 * var(--border-width));
	}

	.archive {
		display: flex;
	}

	.phone .archive {
		margin-top: var(--space-6);
	}

	.phone .archive :global(button) {
		flex: 1;
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

	.heading {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
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
