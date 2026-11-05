<script lang="ts">
	import { ago } from '#lib/chat/sessions.ts';
	import Text from '#lib/chat/Text.svelte';
	import { rigs } from '#lib/config.svelte.ts';
	import type { Chat, Device, Epic, Finding, Review, Reviewer, Ticket, TicketTag } from '#lib/device.svelte.ts';
	import type { Selection } from '#lib/files.svelte.ts';
	import Blockers from '#lib/epics/Blockers.svelte';
	import Tags from '#lib/epics/Tags.svelte';
	import Progress from '#lib/epics/Progress.svelte';
	import TicketDetail from '#lib/epics/Ticket.svelte';
	import { label, land, marks, record, resolved, toggle, tones, type Edit, type OpenEpic } from '#lib/epics.svelte.ts';
	import { form } from '#lib/form.ts';
	import { painted } from '#lib/paint.svelte.ts';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import Select from '#lib/ui/Select.svelte';
	import TextField from '#lib/ui/TextField.svelte';
	import { Popover } from 'bits-ui';
	import { onMount, type Component } from 'svelte';
	import CaretDown from '~icons/ph/caret-down';
	import CaretLeft from '~icons/ph/caret-left';
	import CaretRight from '~icons/ph/caret-right';
	import FileText from '~icons/ph/file-text';
	import GitBranch from '~icons/ph/git-branch';
	import Minus from '~icons/ph/minus';
	import Pause from '~icons/ph/pause';
	import Play from '~icons/ph/play';
	import Plus from '~icons/ph/plus';
	import SealCheck from '~icons/ph/seal-check';
	import SlidersHorizontal from '~icons/ph/sliders-horizontal';
	import Stop from '~icons/ph/stop';
	import TerminalWindow from '~icons/ph/terminal-window';

	type Props = {
		open: OpenEpic;
		epic?: Epic;
		loaded: boolean;
		active: boolean;
		device: Device;
		chats: Chat[];
		onchat: (chatId: string) => void;
		oncompare: (selection: Selection) => void;
	};

	type Page = 'brief' | 'setup' | 'worktree' | 'review';

	let { open, epic, loaded, active, device, chats, onchat, oncompare }: Props = $props();

	const phone = $derived(form() === 'phone');
	const tickets = $derived(epic?.tickets ?? []);
	const ticket = $derived(tickets.find((ticket) => ticket.number === open.ticket));
	const page = $derived.by((): Page | undefined => {
		if (ticket) return;
		return (['brief', 'setup', 'worktree', 'review'] as const).find((target) => target === open.page) ?? (phone ? undefined : 'brief');
	});
	const settled = $derived(tickets.filter((ticket) => resolved(ticket.status)));
	const groups = $derived([
		{
			name: 'Working',
			tickets: tickets.filter((ticket) => ticket.status === 'in-progress' || ticket.status === 'merging').sort((a, b) => Number(needsYou(b)) - Number(needsYou(a)))
		},
		{ name: 'Frontier', tickets: tickets.filter((ticket) => ticket.status === 'not-started' && !pending(ticket).length) },
		{ name: 'Blocked', tickets: tickets.filter((ticket) => ticket.status === 'not-started' && pending(ticket).length > 0) }
	]);
	const lines = $derived(epic?.brief.trimEnd().split('\n').length ?? 0);
	const next = $derived(Math.max(0, ...tickets.map((ticket) => ticket.number)) + 1);
	const choices = $derived(rigs());
	const rig = $derived(choices.some((choice) => choice.name === open.rig) ? open.rig : choices[0]?.name);
	const merges = $derived(tickets.filter((ticket) => ticket.status === 'merged').length);
	const unresolved = $derived(tickets.filter((ticket) => !resolved(ticket.status)));
	const unlandable = $derived.by(() => {
		if (epic?.run || tickets.some((ticket) => ticket.status === 'merging')) return 'Available once no Run is live and nothing is merging';
		if (!epic?.integration) return epic?.landed ? 'Nothing is left to land' : 'Available once the first Ticket starts';
	});
	const standing = $derived.by(() => {
		if (!epic?.landed) return [];
		const kept = tickets.filter((ticket) => ticket.worktree && ticket.attention && resolved(ticket.status));
		return [...(epic.kept ? [epic.kept] : []), ...kept.map((ticket) => `${epic.slug}/${ticket.number}: ${ticket.attention}`)];
	});
	const authors = { agent: 'Agent', david: 'David', stampede: 'Stampede' };
	const working = $derived(tickets.filter((ticket) => ticket.status === 'in-progress'));
	const named = (name?: string) => choices.some((choice) => choice.name === name);
	const reviewers = $derived(epic?.run ? (epic.run.reviewers ?? []) : open.reviewers.filter(named));
	const remediator = $derived(epic?.run?.remediator ?? (named(open.remediator) ? open.remediator : rig));
	const review = $derived(epic?.review);
	const reviewing = $derived(!!review && !review.ended);
	const findings = $derived(epic?.findings ?? []);
	const lineup = $derived.by((): Reviewer[] => {
		if (!review) return [];
		const waiting = (reviewing ? reviewers : []).map((rig, index) => ({ number: index + 1, rig }));
		const unstarted = waiting.filter(({ number }) => !review.reviewers.some((reviewer) => reviewer.number === number));
		return [...review.reviewers, ...unstarted].sort((a, b) => a.number - b.number);
	});
	const summary = $derived.by(() => {
		const parallel = epic?.run?.parallel ?? open.parallel;
		const reviewed = reviewers.length === 1 ? `, ${reviewers[0]} reviews` : reviewers.length ? `, ${reviewers.length} Reviewers` : '';
		return `${epic?.run?.rig ?? rig ?? 'No Rig'} ×${parallel}${reviewed}`;
	});

	let draft = $state('');
	let title = $state('');
	let blockers = $state<number[]>([]);
	let tags = $state<TicketTag[]>([]);
	let setup = $state<string>();
	let error = $state<string>();
	let saving = $state(false);
	let archiving = $state(false);
	let stopping = $state(false);
	let refused = $state<string>();
	let confirming = $state(false);
	let landing = $state(false);
	let unlanded = $state<string>();
	let returnTo: OpenEpic['page'];

	onMount(() => painted(open.id));

	function select(number: number) {
		open.page = undefined;
		open.ticket = number;
	}

	function show(target?: Page) {
		open.ticket = undefined;
		open.page = target;
		setup = undefined;
	}

	function edit(page: 'edit-brief' | 'edit-ticket' | 'new-ticket', text: string) {
		draft = text;
		title = '';
		blockers = [];
		tags = [];
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
			save({ kind: 'ticket.added', ticket: next, title: title.trim(), body: draft, blockers, tags }, back);
		}
	}

	function saveSetup(event: SubmitEvent) {
		event.preventDefault();
		if (setup !== undefined) save({ kind: 'setup.set', command: setup.trim() }, () => (setup = undefined));
	}

	async function run(edit: Edit) {
		if (epic) refused = await record(epic.slug, edit);
	}

	function startRun() {
		open.showRun = false;
		if (rig) run({ kind: 'run.started', rig, parallel: open.parallel, reviewers, remediator: reviewers.length ? remediator : undefined });
	}

	function stopRun() {
		if (working.length) stopping = true;
		else run({ kind: 'run.stopped' });
	}

	function chooseRig(name: string) {
		if (epic?.run) run({ kind: 'run.changed', rig: name });
		else open.rig = name;
	}

	function toggleReviewer(name: string) {
		const chosen = reviewers.includes(name) ? reviewers.filter((other) => other !== name) : [...reviewers, name];
		if (epic?.run) run({ kind: 'run.changed', reviewers: chosen, remediator: chosen.length ? remediator : undefined });
		else open.reviewers = chosen;
	}

	function chooseRemediator(name: string) {
		if (epic?.run) run({ kind: 'run.changed', remediator: name });
		else open.remediator = name;
	}

	function limit(parallel: number) {
		if (parallel < 1) return;
		if (epic?.run) run({ kind: 'run.changed', parallel });
		else open.parallel = parallel;
	}

	async function landEpic() {
		if (landing || !epic) return;
		landing = true;
		unlanded = await land(epic.slug);
		landing = false;
		if (!unlanded) confirming = false;
	}

	function chatOf(ticket: Ticket) {
		return chats.find((chat) => chat.id === ticket.claim?.chat);
	}

	function status(chatId?: string) {
		return chats.find((chat) => chat.id === chatId)?.status ?? 'closed';
	}

	function pending(ticket: Ticket) {
		return ticket.blockers.filter((number) => !tickets.some((other) => other.number === number && resolved(other.status)));
	}

	function needsYou(ticket: Ticket) {
		return !!ticket.attention || chatOf(ticket)?.status === 'waiting';
	}

	function reviewState(review: Review, findings: Finding[]) {
		const remediator = review.remediator;
		if (remediator?.commit) return `Remediated at ${remediator.commit.slice(0, 12)}`;
		if (remediator?.failed) return 'Remediation failed';
		if (!review.ended) return remediator ? 'Remediating' : 'Reviewing';
		const finished = review.reviewers.length > 0 && review.reviewers.every((reviewer) => reviewer.reported || reviewer.failed);
		return finished && !remediator && findings.every((finding) => finding.judgment) ? 'Nothing found' : 'Stopped';
	}

	function reviewerState(reviewer: Reviewer) {
		if (reviewer.failed) return 'failed';
		if (reviewer.reported) return 'reported';
		if (!reviewer.chat) return 'waiting';
		return review?.ended ? 'stopped' : status(reviewer.chat);
	}

	function remediatorState(remediator: NonNullable<Review['remediator']>) {
		if (remediator.failed) return 'failed';
		if (remediator.commit) return 'remediated';
		return review?.ended ? 'stopped' : status(remediator.chat);
	}

	function verdict(finding: Finding) {
		if (!finding.judgment) return { label: 'Not judged', tone: 'var(--status-waiting)' };
		return finding.judgment.accepted ? { label: 'Accepted', tone: 'var(--status-done)' } : { label: 'Rejected', tone: 'var(--muted-fg)' };
	}
</script>

{#snippet back(title: string, action?: { label: string; onclick: () => void })}
	<header class="top">
		<IconButton label="Back to {open.slug}" onclick={() => show()}><CaretLeft /></IconButton>
		<h2>{title}</h2>
		{#if action}<button class="link" onclick={action.onclick}>{action.label}</button>{/if}
	</header>
{/snippet}

{#snippet editing(title: string, action: string, ready: boolean)}
	<header class="top editing">
		<h2>{title}</h2>
		<Button onclick={() => (open.page = returnTo)}>Cancel</Button>
		<Button variant="primary" disabled={saving || !ready} onclick={saveDraft}>{action}</Button>
	</header>
	{#if error}<p class="error" role="alert">{error}</p>{/if}
{/snippet}

{#snippet markdown(label: string, autofocus = true)}
	{#await import('#lib/editor/MarkdownEditor.svelte') then { default: MarkdownEditor }}
		<MarkdownEditor bind:text={draft} {label} {autofocus} onsave={saveDraft} />
	{/await}
{/snippet}

{#snippet brief(epic: Epic)}
	<Text item={{ kind: 'text', text: epic.brief, streaming: false, phase: 'final' }} />
{/snippet}

{#snippet command(epic: Epic)}
	{#if setup !== undefined}
		<form class="setup" onsubmit={saveSetup}>
			<TextField mono bind:value={setup} aria-label="Setup Command" placeholder="pnpm install" spellcheck="false" autocomplete="off" enterkeyhint="done" />
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
		<Select value={epic.run?.rig ?? rig} onchange={(event) => chooseRig(event.currentTarget.value)}>
			{#each choices as choice (choice.name)}
				<option value={choice.name}>{choice.name}</option>
			{/each}
		</Select>
	</label>
	<span class="setting" role="group" aria-label="Parallel">
		<span aria-hidden="true">Parallel</span>
		<IconButton label="Fewer" disabled={parallel <= 1} onclick={() => limit(parallel - 1)}><Minus /></IconButton>
		<output class="limit">{parallel}</output>
		<IconButton label="More" onclick={() => limit(parallel + 1)}><Plus /></IconButton>
	</span>
	<span class="setting">
		<span aria-hidden="true">Reviewers</span>
		<Menu items={choices.map((choice) => ({ label: choice.name, checked: reviewers.includes(choice.name), keepOpen: true, onselect: () => toggleReviewer(choice.name) }))}>
			{#snippet trigger(props)}
				<button class="picker" {...props} disabled={reviewing || !choices.length} aria-label="Reviewers: {reviewers.join(', ') || 'none'}">
					<span>{reviewers.join(', ') || 'None'}</span>
					<CaretDown />
				</button>
			{/snippet}
		</Menu>
	</span>
	{#if reviewers.length}
		<label class="setting">
			<span>Remediator</span>
			<Select value={remediator} disabled={reviewing && !!review?.remediator} onchange={(event) => chooseRemediator(event.currentTarget.value)}>
				{#each choices as choice (choice.name)}
					<option value={choice.name}>{choice.name}</option>
				{/each}
			</Select>
		</label>
	{/if}
{/snippet}

{#snippet liveLabel(live: NonNullable<Epic['run']>)}
	<span class="live" class:paused={!!live.paused}>{live.paused ? 'Run paused' : reviewing ? (review?.remediator ? 'Remediating' : 'Reviewing') : 'Run live'}</span>
{/snippet}

{#snippet runNotes(epic: Epic)}
	{#if epic.run?.paused?.reason === 'storage'}
		<p class="note error" role="status">{epic.run.paused.message} New Tickets wait; active Chats continue.</p>
	{/if}
	{#if !epic.run && epic.setup === null}<p class="note">Record a Setup Command before starting a Run.</p>{/if}
	{#if refused}<p class="note error" role="alert">{refused}</p>{/if}
{/snippet}

{#snippet startButton(epic: Epic)}
	<Button variant="primary" disabled={!rig || epic.setup === null || open.savingRigs.length > 0} onclick={startRun}><Play /> Start Run</Button>
{/snippet}

{#snippet runBar(epic: Epic)}
	{@const live = epic.run}
	<section class="run" aria-label="Run">
		<div class="controls">
			{#if live}{@render liveLabel(live)}{/if}
			<Popover.Root bind:open={open.showRun}>
				<Popover.Trigger>
					{#snippet child({ props })}
						<button class="picker" {...props} aria-label="Run settings, {summary}">
							<span>{summary}</span>
							<CaretDown />
						</button>
					{/snippet}
				</Popover.Trigger>
				<Popover.Content sideOffset={6} align="end" collisionPadding={8}>
					{#snippet child({ wrapperProps, props })}
						<div {...wrapperProps}>
							<div {...props} class="popover" role="dialog" aria-label="Run settings">{@render settings(epic)}</div>
						</div>
					{/snippet}
				</Popover.Content>
			</Popover.Root>
			{#if live}
				{#if live.paused}
					<IconButton label="Resume" onclick={() => run({ kind: 'run.resumed' })}><Play /></IconButton>
				{:else}
					<IconButton label="Pause" onclick={() => run({ kind: 'run.paused', reason: 'david' })}><Pause /></IconButton>
				{/if}
				<IconButton label="Stop" onclick={stopRun}><Stop /></IconButton>
			{:else}
				{@render startButton(epic)}
			{/if}
		</div>
		{@render runNotes(epic)}
	</section>
{/snippet}

{#snippet runStrip(epic: Epic)}
	{@const live = epic.run}
	<section class="run strip" aria-label="Run">
		{#if live}
			<div class="controls">
				{@render liveLabel(live)}
				<span class="summary">{summary}</span>
				{#if live.paused}
					<IconButton label="Resume" onclick={() => run({ kind: 'run.resumed' })}><Play /></IconButton>
				{:else}
					<IconButton label="Pause" onclick={() => run({ kind: 'run.paused', reason: 'david' })}><Pause /></IconButton>
				{/if}
				<IconButton label="Stop" onclick={stopRun}><Stop /></IconButton>
				<IconButton label="Run settings" aria-expanded={open.showRun} onclick={() => (open.showRun = !open.showRun)}><SlidersHorizontal /></IconButton>
			</div>
		{/if}
		{#if !live || open.showRun}
			<div class="settings">{@render settings(epic)}</div>
		{/if}
		{#if !live}{@render startButton(epic)}{/if}
		{@render runNotes(epic)}
	</section>
{/snippet}

{#snippet progress()}
	{#if epic && tickets.length}
		<div class="progress">
			<span class="meter"><Progress {epic} segmented /></span>
			<span>{settled.length} of {tickets.length} resolved</span>
		</div>
	{/if}
{/snippet}

{#snippet compareButton(epic: Epic)}
	{#if epic.integration && epic.base}
		{@const target = epic.base}
		<Button onclick={() => oncompare({ target, source: `stampede/${epic.slug}/epic` })}>Compare with base</Button>
	{/if}
{/snippet}

{#snippet landButton(epic: Epic)}
	<Button variant="primary" disabled={!!unlandable} title={phone ? undefined : unlandable} onclick={() => ((unlanded = undefined), (confirming = true))}>
		{epic.base ? `Land into ${epic.base}` : 'Land'}
	</Button>
{/snippet}

{#snippet worktree(epic: Epic)}
	<section aria-label="Integration Worktree" class="page">
		{#if !phone}
			<div class="heading">
				<h2>Integration Worktree</h2>
				{#if epic.integration}
					<Button onclick={() => epic.integration && device.send({ type: 'project.open', path: epic.integration })}>Open</Button>
				{/if}
			</div>
		{/if}
		{#if epic.integration}
			<p class="unset">{merges} {merges === 1 ? 'merge' : 'merges'}{#if epic.base}{' '}· cut from {epic.base}{/if}</p>
		{:else if !epic.landed}
			<p class="unset">Not cut yet; the first Ticket to start cuts it.</p>
		{/if}
		{#if epic.landed}
			<p class="unset">Landed into {epic.landed.base} at <span class="head">{epic.landed.head.slice(0, 12)}</span></p>
		{/if}
		{#if standing.length}
			<div class="standing" role="status">
				<h3>Left standing</h3>
				<ul>
					{#each standing as reason, index (index)}<li>{reason}</li>{/each}
				</ul>
			</div>
		{/if}
		{#each epic.notes ?? [] as note, index (index)}
			<div class="entry">
				<span class="meta">{authors[note.author]} · {ago(note.at)}</span>
				<p>{note.text}</p>
			</div>
		{/each}
	</section>
{/snippet}

{#snippet reviewSection(review: Review | undefined)}
	<section aria-label="Review" class="page review">
		<div class="heading">
			{#if !phone}<h2>Review</h2>{/if}
			{#if review}<span class="unset">{reviewState(review, findings)}</span>{/if}
		</div>
		{#if review}
			<ul class="crew">
				{#each lineup as reviewer (reviewer.number)}
					<li>
						<span class="who">Reviewer {reviewer.number}</span>
						<span class="rig">{reviewer.rig}</span>
						<span class="state" class:flagged={!!reviewer.failed}>{reviewerState(reviewer)}</span>
						{#if reviewer.chat && chats.some((chat) => chat.id === reviewer.chat)}
							<button class="link" onclick={() => reviewer.chat && onchat(reviewer.chat)}>Open</button>
						{/if}
						{#if reviewer.failed}<p class="why">{reviewer.failed}</p>{/if}
					</li>
				{/each}
				{#if review.remediator}
					{@const remediator = review.remediator}
					<li>
						<span class="who">Remediator</span>
						<span class="rig">{remediator.rig}</span>
						<span class="state" class:flagged={!!remediator.failed}>{remediatorState(remediator)}</span>
						{#if remediator.chat && chats.some((chat) => chat.id === remediator.chat)}
							<button class="link" onclick={() => remediator.chat && onchat(remediator.chat)}>Open</button>
						{/if}
						{#if remediator.failed}<p class="why">{remediator.failed}</p>{/if}
					</li>
				{/if}
			</ul>
		{/if}
		{#each findings as finding (finding.number)}
			{@const judged = verdict(finding)}
			<details class="finding">
				<summary>
					<span class="number">{finding.number}</span>
					<span class="title">{finding.title}</span>
					<span class="verdict" style:--tone={judged.tone}>{judged.label}</span>
				</summary>
				<div class="finding-body">
					<p class="meta">{finding.rig} · {ago(finding.at)}</p>
					<Text item={{ kind: 'text', text: finding.body, streaming: false, phase: 'final' }} />
					{#if finding.judgment}<p class="reason"><strong>{judged.label}:</strong> {finding.judgment.reason}</p>{/if}
				</div>
			</details>
		{/each}
	</section>
{/snippet}

{#snippet pageView(epic: Epic, page?: Page)}
	{#if page === 'brief'}
		<section aria-label="Brief" class="page">
			{#if !phone}
				<div class="heading">
					<h2>Brief</h2>
					<Button onclick={() => edit('edit-brief', epic.brief)}>Edit</Button>
				</div>
			{/if}
			{@render brief(epic)}
		</section>
	{:else if page === 'setup'}
		<section aria-label="Setup Command" class="page">
			{#if !phone}
				<div class="heading">
					<h2>Setup Command</h2>
					{#if setup === undefined}<Button onclick={() => editSetup(epic.setup)}>Edit</Button>{/if}
				</div>
			{/if}
			{@render command(epic)}
		</section>
	{:else if page === 'worktree'}
		{@render worktree(epic)}
	{:else if page === 'review'}
		{@render reviewSection(review)}
	{/if}
{/snippet}

{#snippet pageItem(target: Page, Icon: Component, name: string, meta: string, mono = false)}
	<button class="item" aria-current={page === target ? 'page' : undefined} onclick={() => show(target)}>
		<Icon />
		<span class="title">{name}</span>
		<span class="meta" class:mono>{meta}</span>
	</button>
{/snippet}

{#snippet row(ticket: Ticket)}
	{@const Mark = marks[ticket.status]}
	{@const chat = chatOf(ticket)}
	{@const after = pending(ticket)}
	<button class="item" class:resolved={resolved(ticket.status)} aria-current={!phone && open.ticket === ticket.number ? 'true' : undefined} onclick={() => select(ticket.number)}>
		<span class="mark" style:color={tones[ticket.status]}><Mark /></span>
		<span class="number">{ticket.number}</span>
		<span class="title">{ticket.title}</span>
		<span class="label" class:shown={ticket.status === 'merging'} style:color={tones[ticket.status]}>{label(ticket.status)}</span>
		{#if ticket.attention}
			<span class="meta flagged">needs attention</span>
		{:else if ticket.status === 'in-progress'}
			<span class="meta">
				{ticket.claim?.rig}{#if ticket.claim?.edited}{' '}· edited{/if}
				<span class="dot" style:background={chat ? `var(--status-${chat.status})` : 'var(--muted-fg)'} title={ticket.releasing ? 'stopping' : (chat?.status ?? 'closed')}></span>
			</span>
		{:else if ticket.status === 'not-started' && (after.length || ticket.rig)}
			<span class="meta">{after.length ? `after ${after.join(', ')}` : ticket.rig}</span>
		{/if}
	</button>
{/snippet}

{#snippet list(epic: Epic)}
	{@const setupText = epic.setup === null ? 'Not set yet' : epic.setup || 'Nothing to run'}
	<nav class="pages" aria-label="Epic">
		{@render pageItem('brief', FileText, 'Brief', `${lines} ${lines === 1 ? 'line' : 'lines'}`)}
		{@render pageItem('setup', TerminalWindow, phone ? 'Setup' : 'Setup Command', setupText, !!epic.setup)}
		{@render pageItem('worktree', GitBranch, phone ? 'Worktree' : 'Integration Worktree', epic.landed ? 'Landed' : epic.integration ? `${merges} ${merges === 1 ? 'merge' : 'merges'}` : 'Not cut yet')}
		{#if review || findings.length}
			{@render pageItem('review', SealCheck, 'Review', review ? reviewState(review, findings) : `${findings.length} ${findings.length === 1 ? 'Finding' : 'Findings'}`)}
		{/if}
	</nav>
	<section aria-label="Tickets" class="tickets">
		{#each groups as group (group.name)}
			{#if group.tickets.length}
				<h3 class="group">{group.name}<span class="count">{group.tickets.length}</span></h3>
				{#each group.tickets as ticket (ticket.number)}{@render row(ticket)}{/each}
			{/if}
		{/each}
		{#if settled.length}
			<button class="group fold" aria-expanded={open.showResolved} onclick={() => (open.showResolved = !open.showResolved)}>
				<CaretRight />Resolved<span class="count">{settled.length}</span>
			</button>
			{#if open.showResolved}
				{#each settled as ticket (ticket.number)}{@render row(ticket)}{/each}
			{/if}
		{/if}
		{#if !tickets.length}<p class="unset">No Tickets yet.</p>{/if}
		<button class="add" onclick={() => edit('new-ticket', '')}><Plus /> Add a Ticket</button>
	</section>
	{#if !epic.archived}<button class="archive" onclick={() => ((error = undefined), (archiving = true))}>Archive Epic</button>{/if}
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
			<TextField bind:value={title} aria-label="Title" placeholder="Title" autocomplete="off" />
			<div class="field">
				<span>Tags</span>
				<Tags {tags} onchange={(value) => (tags = value)} />
			</div>
			{#if tickets.length}
				<div class="field">
					<span>Blocked by</span>
					<Blockers {tickets} {blockers} ontoggle={(number) => (blockers = toggle(blockers, number))} />
				</div>
			{/if}
		</div>
		{@render markdown('Body', false)}
	{:else if phone && ticket}
		{@render back(`${epic.slug}/${ticket.number}`, { label: 'Edit', onclick: () => edit('edit-ticket', ticket.body) })}
		<div class="scroll">
			{#key ticket.number}
				<TicketDetail {epic} {ticket} onedit={() => edit('edit-ticket', ticket.body)} {open} chat={chatOf(ticket)} {device} {onchat} onselect={select} />
			{/key}
		</div>
	{:else if phone && page === 'brief'}
		{@render back('Brief', { label: 'Edit', onclick: () => edit('edit-brief', epic.brief) })}
		<div class="scroll">{@render pageView(epic, page)}</div>
	{:else if phone && page === 'setup'}
		{@render back('Setup Command', setup === undefined ? { label: 'Edit', onclick: () => editSetup(epic.setup) } : undefined)}
		<div class="scroll">{@render pageView(epic, page)}</div>
	{:else if phone && page === 'worktree'}
		{@render back('Integration Worktree', epic.integration ? { label: 'Open', onclick: () => epic.integration && device.send({ type: 'project.open', path: epic.integration }) } : undefined)}
		<div class="scroll">{@render pageView(epic, page)}</div>
	{:else if phone && page === 'review'}
		{@render back('Review')}
		<div class="scroll">{@render pageView(epic, page)}</div>
	{:else if phone}
		<div class="scroll">
			<header class="head">
				<div class="name">
					<h1>{epic.slug}</h1>
					{#if epic.archived}<span class="badge">Archived</span>{/if}
				</div>
				<p class="title">{epic.title}</p>
				{@render progress()}
			</header>
			{@render runStrip(epic)}
			{@render list(epic)}
		</div>
		<footer class="land">
			{@render compareButton(epic)}
			{@render landButton(epic)}
			{#if unlandable}<p class="hint">{unlandable}</p>{/if}
		</footer>
	{:else}
		<header class="bar">
			<div class="name">
				<h1>{epic.slug}</h1>
				{#if epic.archived}<span class="badge">Archived</span>{/if}
				<span class="title">{epic.title}</span>
			</div>
			{@render progress()}
			{@render runBar(epic)}
			{@render compareButton(epic)}
			{@render landButton(epic)}
		</header>
		<div class="split">
			<div class="list">{@render list(epic)}</div>
			<div class="pane">
				{#if ticket}
					{#key ticket.number}
						<TicketDetail {epic} {ticket} onedit={() => edit('edit-ticket', ticket.body)} {open} chat={chatOf(ticket)} {device} {onchat} onselect={select} />
					{/key}
				{:else}
					{@render pageView(epic, page)}
				{/if}
			</div>
		</div>
	{/if}
</div>

<Dialog
	bind:open={confirming}
	title={epic?.base ? `Land ${open.slug} into ${epic.base}?` : `Land ${open.slug}?`}
	description="Stampede merges the Epic branch into {epic?.base ?? 'its base'} in the Project's own checkout, then removes the Integration Worktree and the Epic branch if they are clean. It refuses, changing nothing, if the checkout is on another branch, has uncommitted changes, or would conflict."
>
	{#if unresolved.length}
		<div class="unresolved">
			<p>Still unresolved:</p>
			<ul>
				{#each unresolved as ticket (ticket.number)}
					{@const Mark = marks[ticket.status]}
					<li><span class="number">{ticket.number}</span> {ticket.title} · <span class="mark" style:color={tones[ticket.status]}><Mark />{label(ticket.status)}</span></li>
				{/each}
			</ul>
		</div>
	{:else}
		<p class="unresolved">Every Ticket is merged or skipped.</p>
	{/if}
	{#if unlanded}<p class="error" role="alert">{unlanded}</p>{/if}
	{#snippet actions()}
		<Button onclick={() => (confirming = false)}>Cancel</Button>
		<Button variant="primary" disabled={landing} onclick={landEpic}>{landing ? 'Landing…' : 'Land'}</Button>
	{/snippet}
</Dialog>

<Dialog
	bind:open={archiving}
	title="Archive {open.slug}?"
	description="It is hidden from the active list. Use Show archived to view it later. Nothing in Stampede unarchives it yet."
>
	{#if error}<p class="error" role="alert">{error}</p>{/if}
	{#snippet actions()}
		<Button onclick={() => (archiving = false)}>Cancel</Button>
		<Button variant="primary" disabled={saving} onclick={() => save({ kind: 'epic.archived' }, () => (archiving = false))}>Archive</Button>
	{/snippet}
</Dialog>

<Dialog
	bind:open={stopping}
	title="Stop the Run?"
	description={working.length === 1
		? 'Its Chat stops and its Ticket goes back to not started, keeping its worktree and Agent session for the next Run.'
		: `Its ${working.length} Chats stop and their Tickets go back to not started, each keeping its worktree and Agent session for the next Run.`}
>
	{#snippet actions()}
		<Button onclick={() => (stopping = false)}>Cancel</Button>
		<Button variant="primary" onclick={() => ((stopping = false), run({ kind: 'run.stopped' }))}>Stop</Button>
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

	header.top {
		display: flex;
		flex: none;
		align-items: center;
		gap: var(--space-1);
		padding: var(--space-1);
		border-bottom: var(--border-width) solid var(--border);
	}

	header.top h2 {
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

	header.top .link {
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

	.bar {
		display: flex;
		flex: none;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-3) var(--space-6);
		padding: var(--space-3) var(--space-4) var(--space-3) var(--space-6);
		border-bottom: var(--border-width) solid var(--border);
	}

	.name {
		display: flex;
		align-items: baseline;
		gap: var(--space-3);
		min-width: 0;
	}

	.bar .name {
		flex: 1 1 8rem;
	}

	h1 {
		flex: none;
		margin: 0;
		font-size: var(--font-size-lg);
	}

	.name .title {
		min-width: 0;
		overflow: hidden;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.badge {
		flex: none;
		padding: 0 var(--space-2);
		border: var(--border-width) solid var(--border);
		border-radius: 1em;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.progress {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}

	.meter {
		display: block;
		width: 10rem;
	}

	.run {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		gap: var(--space-1);
		max-width: 100%;
	}

	.bar .run {
		padding-left: var(--space-6);
		border-left: var(--border-width) solid var(--border);
	}

	.controls {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-width: 0;
	}

	.live {
		display: flex;
		flex: none;
		align-items: center;
		gap: var(--space-2);
		font-size: var(--font-size-sm);
		font-weight: 600;
	}

	.live::before {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--status-done);
		content: '';
	}

	.live.paused::before {
		background: var(--status-waiting);
	}

	.note {
		max-width: 28rem;
		margin: 0;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		overflow-wrap: anywhere;
	}

	.note.error {
		color: var(--ansi-1);
	}

	.popover {
		z-index: 50;
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		padding: var(--space-4);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--surface-2);
		box-shadow: var(--shadow-float);
		color: var(--color-fg);
		font-family: var(--font-ui);
		font-size: var(--font-size-md);
		outline: none;
	}

	.setting {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		color: var(--muted-fg);
	}

	.setting > span:first-child {
		flex: 1;
		min-width: 7rem;
	}

	.popover .setting > :global(select),
	.popover .picker {
		width: 12rem;
		max-width: none;
		justify-content: space-between;
	}

	.limit {
		min-width: 1.5em;
		color: var(--color-fg);
		font-family: var(--font-mono);
		text-align: center;
	}

	.picker {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		max-width: 20rem;
		min-height: var(--target-min);
		padding: 0 var(--space-2);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-1);
		color: var(--color-fg);
		font: inherit;
		font-size: var(--font-size-sm);
		cursor: pointer;
	}

	.picker span {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.picker :global(svg) {
		flex: none;
		color: var(--muted-fg);
	}

	.picker:disabled {
		color: var(--muted-fg);
		cursor: default;
	}

	.picker:focus-visible {
		outline: var(--border-width) solid var(--accent);
	}

	.split {
		display: grid;
		flex: 1;
		grid-template-columns: clamp(17rem, 34%, 24rem) minmax(0, 1fr);
		min-height: 0;
	}

	.list {
		display: flex;
		flex-direction: column;
		gap: 1px;
		min-height: 0;
		overflow: auto;
		padding: var(--space-3) var(--space-2);
		border-right: var(--border-width) solid var(--border);
	}

	.pane {
		min-width: 0;
		min-height: 0;
		overflow: auto;
		padding: var(--space-6) calc(2 * var(--space-6));
		container-type: inline-size;
	}

	.pages,
	.tickets {
		display: flex;
		flex-direction: column;
		gap: 1px;
	}

	.pages {
		margin-bottom: var(--space-1);
		padding-bottom: var(--space-3);
		border-bottom: var(--border-width) solid var(--border);
	}

	.item {
		position: relative;
		display: flex;
		align-items: center;
		gap: var(--space-3);
		width: 100%;
		min-height: calc(var(--target-min) + var(--space-2));
		padding: 0 var(--space-3);
		border: none;
		border-radius: var(--radius-md);
		background: none;
		color: var(--color-fg);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}

	.item:hover {
		background: var(--hover);
	}

	.item[aria-current] {
		background: var(--surface-2);
	}

	.item > :global(svg),
	.mark {
		display: flex;
		flex: none;
		color: var(--muted-fg);
	}

	.item .title {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.item.resolved {
		color: var(--muted-fg);
	}

	.number {
		color: var(--muted-fg);
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		font-variant-numeric: tabular-nums;
	}

	.meta {
		color: var(--muted-fg);
	}

	.item .meta {
		display: flex;
		flex: none;
		align-items: center;
		gap: var(--space-2);
		max-width: 45%;
		overflow: hidden;
		font-size: var(--font-size-sm);
		white-space: nowrap;
	}

	.mono {
		font-family: var(--font-mono);
	}

	.flagged {
		color: var(--status-waiting);
	}

	.label {
		position: absolute;
		width: 1px;
		height: 1px;
		overflow: hidden;
		clip-path: inset(50%);
		white-space: nowrap;
	}

	.label.shown {
		position: static;
		width: auto;
		height: auto;
		clip-path: none;
		font-size: var(--font-size-sm);
	}

	.dot {
		flex: none;
		width: 7px;
		height: 7px;
		border-radius: 50%;
	}

	.group {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		margin: 0;
		padding: var(--space-4) var(--space-3) var(--space-1);
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		font-weight: 600;
	}

	.count {
		color: color-mix(in oklab, var(--muted-fg) 70%, var(--color-bg));
		font-family: var(--font-mono);
		font-weight: 400;
	}

	.fold {
		border: none;
		background: none;
		font-family: inherit;
		text-align: left;
		cursor: pointer;
	}

	.fold :global(svg) {
		transition: rotate var(--dur-fast);
	}

	.fold[aria-expanded='true'] :global(svg) {
		rotate: 90deg;
	}

	.tickets > .unset {
		padding: var(--space-3);
	}

	.add,
	.archive {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		min-height: calc(var(--target-min) + var(--space-2));
		padding: 0 var(--space-3);
		border: none;
		border-radius: var(--radius-md);
		background: none;
		color: var(--muted-fg);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}

	.add {
		margin-top: var(--space-2);
	}

	.archive {
		align-self: flex-start;
		margin-top: auto;
		padding-top: var(--space-6);
		font-size: var(--font-size-sm);
	}

	.add:hover,
	.archive:hover {
		color: var(--color-fg);
	}

	.add:hover {
		background: var(--hover);
	}

	.item:focus-visible,
	.fold:focus-visible,
	.add:focus-visible,
	.archive:focus-visible,
	.link:focus-visible {
		outline: var(--border-width) solid var(--accent);
		outline-offset: calc(-1 * var(--border-width));
	}

	.page {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		max-width: 72ch;
	}

	.heading {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3);
	}

	.page h2 {
		margin: 0;
		font-size: var(--font-size-lg);
	}

	.review .heading {
		justify-content: flex-start;
	}

	.crew {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		margin: 0;
		padding: 0;
		font-size: var(--font-size-sm);
		list-style: none;
	}

	.crew li {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 0 var(--space-3);
	}

	.who {
		min-width: 6.5rem;
	}

	.crew .rig,
	.crew .state {
		color: var(--muted-fg);
	}

	.crew .state.flagged {
		color: var(--status-waiting);
	}

	.crew .link {
		margin-left: auto;
	}

	.why {
		flex-basis: 100%;
		margin: 0;
		color: var(--muted-fg);
		overflow-wrap: anywhere;
	}

	.finding {
		border-top: var(--border-width) solid var(--border);
	}

	.finding summary {
		display: flex;
		align-items: baseline;
		gap: var(--space-3);
		min-height: var(--target-min);
		padding: var(--space-2) 0;
		list-style: none;
		cursor: pointer;
	}

	.finding summary::-webkit-details-marker {
		display: none;
	}

	.finding summary:focus-visible {
		outline: var(--border-width) solid var(--accent);
	}

	.finding summary .number {
		flex: none;
		width: 2.5rem;
	}

	.finding .title {
		flex: 1;
		min-width: 0;
		overflow-wrap: anywhere;
	}

	.finding[open] .title {
		font-weight: 600;
	}

	.verdict {
		flex: none;
		color: var(--tone);
		font-size: var(--font-size-sm);
	}

	.finding-body {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		padding: 0 0 var(--space-3) calc(2.5rem + var(--space-3));
		font-size: var(--font-size-sm);
		overflow-wrap: anywhere;
	}

	.phone .finding-body {
		padding-left: 0;
	}

	.finding-body p {
		margin: 0;
	}

	.page .head {
		font-family: var(--font-mono);
	}

	.hint {
		margin: 0;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.standing {
		padding: var(--space-2) var(--space-3);
		border-left: 2px solid var(--status-waiting);
		background: color-mix(in oklab, var(--status-waiting) 10%, transparent);
		font-size: var(--font-size-sm);
		overflow-wrap: anywhere;
	}

	.standing h3 {
		margin: 0;
		font-size: inherit;
	}

	.standing ul {
		margin: 0;
		padding-left: 1.25em;
	}

	.entry {
		padding-left: var(--space-3);
		border-left: 2px solid var(--border);
		font-size: var(--font-size-sm);
	}

	.entry p {
		margin: 0;
		overflow-wrap: anywhere;
	}

	.link {
		padding: 0;
		border: none;
		background: none;
		color: var(--accent);
		font: inherit;
		font-size: var(--font-size-sm);
		cursor: pointer;
	}

	code {
		align-self: flex-start;
		max-width: 100%;
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

	.head .title {
		margin: var(--space-1) 0 var(--space-3);
		color: var(--muted-fg);
	}

	.phone h1 {
		font-size: 1.25em;
	}

	.phone .meter {
		flex: 1;
		width: auto;
	}

	.strip {
		align-items: stretch;
		gap: var(--space-3);
		margin: var(--space-3) 0;
		padding: var(--space-2) var(--space-2) var(--space-2) var(--space-3);
		border-radius: var(--radius-lg);
		background: var(--surface-1);
	}

	.strip:not(:has(> .controls)) {
		padding: var(--space-3);
	}

	.summary {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.settings {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}

	.strip .setting > :global(select),
	.strip .picker {
		flex: 2;
		max-width: none;
		justify-content: space-between;
	}

	.phone .pages {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(6.5rem, 1fr));
		gap: var(--space-2);
		margin-bottom: var(--space-1);
		padding-bottom: var(--space-3);
	}

	.phone .pages .item {
		flex-direction: column;
		align-items: flex-start;
		gap: 2px;
		padding: var(--space-2) var(--space-3);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
	}

	.phone .pages .title {
		flex: none;
		width: 100%;
	}

	.phone .pages .item .meta {
		max-width: 100%;
		font-size: calc(var(--font-size-sm) - 1px);
	}

	.phone .item {
		padding: 0 var(--space-1);
	}

	.phone .group {
		padding-left: var(--space-1);
	}

	.phone .archive {
		align-self: stretch;
		justify-content: center;
	}

	.phone .page {
		max-width: none;
	}

	.land {
		display: flex;
		flex: none;
		flex-direction: column;
		gap: var(--space-1);
		padding: var(--space-2) var(--space-3) var(--space-3);
		border-top: var(--border-width) solid var(--border);
	}

	.land .hint {
		text-align: center;
	}

	.unresolved {
		margin: var(--space-3) 0 0;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.unresolved p {
		margin: 0;
	}

	.unresolved ul {
		margin: var(--space-1) 0 0;
		padding: 0;
		color: var(--color-fg);
		list-style: none;
	}

	.unresolved .mark {
		display: inline;
	}

	.unresolved .mark :global(svg) {
		margin-right: var(--space-1);
		vertical-align: -0.125em;
	}

	.unresolved + .error {
		margin-top: var(--space-3);
	}
</style>
