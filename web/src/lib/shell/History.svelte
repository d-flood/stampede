<script lang="ts">
	import type { Device } from '#lib/device.svelte.ts';
	import { commitFiles, letters, log, message, reword, split, squash, undo, type Change, type Commit } from '#lib/git.ts';
	import { restoreMessage } from '#lib/shell/Changes.svelte';
	import CopyMode from '#lib/shell/CopyMode.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import { onMount } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import ArrowUUpLeft from '~icons/ph/arrow-u-up-left';
	import CaretRight from '~icons/ph/caret-right';
	import Copy from '~icons/ph/copy';
	import DotsThree from '~icons/ph/dots-three';
	import GitMerge from '~icons/ph/git-merge';
	import PencilSimple from '~icons/ph/pencil-simple';

	type Props = { projectId: string; device: Device; onopen(change: Change, commit: Commit): void };

	let { projectId, device, onopen }: Props = $props();

	let commits = $state.raw<Commit[]>();
	let cursor = $state<string>();
	let files = $state<Record<string, Change[]>>({});
	const expanded = new SvelteSet<string>();
	let editing = $state<{ commits: Commit[]; summary: string; description: string; loaded: boolean }>();
	let edits = 0;
	let saving = $state(false);
	let refusal = $state<string>();
	let failure = $state<string>();
	let copying = $state<string>();
	let refreshes = 0;
	let requested: string | undefined;

	onMount(() => {
		refresh();
		return device.listen({
			message(message) {
				if (message.type === 'hello' || (message.type === 'git.changed' && message.projectId === projectId)) refresh();
			}
		});
	});

	async function refresh() {
		const generation = ++refreshes;
		const page = await log(projectId);
		if (generation !== refreshes || (commits && page.commits[0]?.sha === commits[0]?.sha)) return;
		commits = page.commits;
		cursor = page.cursor;
	}

	async function more() {
		const from = cursor;
		if (!from || from === requested) return;
		requested = from;
		const page = await log(projectId, from);
		if (cursor !== from || !commits) return;
		commits = [...commits, ...page.commits];
		cursor = page.cursor;
	}

	function scrolled({ currentTarget: list }: Event & { currentTarget: HTMLElement }) {
		if (list.scrollTop + 2 * list.clientHeight >= list.scrollHeight) more();
	}

	async function toggle(sha: string) {
		if (expanded.delete(sha)) return;
		expanded.add(sha);
		files[sha] ??= await commitFiles(projectId, sha);
	}

	async function edit(range: Commit[]) {
		const generation = ++edits;
		editing = { commits: range, summary: '', description: '', loaded: false };
		refusal = undefined;
		try {
			const messages = await Promise.all(range.toReversed().map(({ sha }) => message(projectId, sha)));
			const [summary, ...rest] = messages.join('\n\n').split('\n');
			if (generation === edits && editing) editing = { commits: range, summary, description: rest.join('\n').replace(/^\n+/, ''), loaded: true };
		} catch (error) {
			refusal = error instanceof Error ? error.message : String(error);
		}
	}

	function squashInto(commit: Commit) {
		const at = commits?.indexOf(commit) ?? -1;
		if (at > 0) edit(commits!.slice(0, at + 1));
	}

	async function save(event?: SubmitEvent) {
		event?.preventDefault();
		if (!editing?.loaded || !editing.summary.trim() || saving) return;
		const { commits: range, summary, description } = editing;
		const text = description.trim() ? `${summary.trim()}\n\n${description}` : summary.trim();
		saving = true;
		refusal = undefined;
		try {
			if (range.length > 1) await squash(projectId, range.at(-1)!.sha, range[0].sha, text);
			else await reword(projectId, range[0].sha, text);
			editing = undefined;
		} catch (error) {
			refusal = error instanceof Error ? error.message : String(error);
		} finally {
			saving = false;
		}
	}

	async function undoCommit() {
		failure = undefined;
		try {
			restoreMessage(projectId, await undo(projectId));
		} catch (error) {
			failure = error instanceof Error ? error.message : String(error);
		}
	}

	function copy(text: string) {
		navigator.clipboard.writeText(text).catch(() => (copying = text));
	}

	function key(event: KeyboardEvent & { currentTarget: HTMLElement }) {
		if (event.defaultPrevented || event.isComposing || event.ctrlKey || event.metaKey || event.altKey) return;
		const row = event.target instanceof HTMLElement ? event.target.closest<HTMLElement>('[data-sha]') : null;
		const rows = [...event.currentTarget.querySelectorAll<HTMLElement>('[data-sha]')];
		const commit = commits?.find(({ sha }) => sha === row?.dataset.sha);
		const at = row ? rows.indexOf(row) : -1;
		const actions: Record<string, () => void> = {
			j: () => rows[Math.min(rows.length - 1, at + 1)]?.focus(),
			ArrowDown: () => rows[Math.min(rows.length - 1, at + 1)]?.focus(),
			k: () => rows[Math.max(0, at - 1)]?.focus(),
			ArrowUp: () => rows[Math.max(0, at - 1)]?.focus(),
			r: () => commit && edit([commit]),
			s: () => commit && squashInto(commit),
			y: () => commit && copy(commit.short),
			Y: () => commit && copy(commit.sha),
			z: undoCommit
		};
		const action = actions[event.key];
		if (!action) return;
		event.preventDefault();
		action();
	}
</script>

<section aria-label="History">
	<header><h2>History</h2></header>
	{#if failure}<p class="failure" role="alert">{failure}</p>{/if}
	<!-- svelte-ignore a11y_no_static_element_interactions (Keys act on the focused commit.) -->
	<div class="list" onscroll={scrolled} onkeydown={key}>
		<ul aria-label="Commits">
			{#each commits ?? [] as commit, index (commit.sha)}
				{@const open = expanded.has(commit.sha)}
				{@const items = [
					{ label: 'Edit Message…', icon: PencilSimple, hint: 'r', onselect: () => edit([commit]) },
					...(index > 0 ? [{ label: 'Squash Newer Commits Into This…', icon: GitMerge, hint: 's', onselect: () => squashInto(commit) }] : []),
					{ label: 'Copy Short Hash', icon: Copy, hint: 'y', onselect: () => copy(commit.short) },
					{ label: 'Copy Full Hash', icon: Copy, hint: 'Y', onselect: () => copy(commit.sha) },
					...(index === 0 ? [{ label: 'Undo Last Commit', icon: ArrowUUpLeft, hint: 'z', onselect: undoCommit }] : [])
				]}
				<li>
					<div class="row">
						<Menu context {items}>
							{#snippet trigger(props)}
								<button
									{...props}
									class="commit"
									data-sha={commit.sha}
									aria-expanded={open}
									title={commit.subject}
									onclick={(event) => {
										if (event.currentTarget.dataset.state !== 'open') toggle(commit.sha);
									}}
								>
									<span class="caret" class:open><CaretRight /></span>
									<span class="text">
										<span class="subject">{commit.subject}</span>
										<span class="meta"><span class="sha">{commit.short}</span> · {commit.author} · {commit.date}</span>
									</span>
								</button>
							{/snippet}
						</Menu>
						<Menu {items}>
							{#snippet trigger(props)}
								<IconButton {...props} label={`Actions for ${commit.short}`}><DotsThree /></IconButton>
							{/snippet}
						</Menu>
					</div>
					{#if open && files[commit.sha]}
						<ul aria-label="Files in {commit.short}">
							{#each files[commit.sha] as change (change.path)}
								{@const { name, dir } = split(change.path)}
								<li>
									<button
										class="file"
										class:deleted={change.kind === 'deleted'}
										style:--git="var(--git-{change.kind})"
										title={change.from ? `${change.from} → ${change.path}` : change.path}
										onclick={() => onopen(change, commit)}
									>
										<span class="letter" aria-hidden="true">{letters[change.kind]}</span>
										<span class="name">{name}</span>
										<span class="dir">{dir}</span>
									</button>
								</li>
							{:else}
								<li class="empty">No files changed</li>
							{/each}
						</ul>
					{/if}
				</li>
			{/each}
		</ul>
		{#if commits?.length === 0}<p>No commits</p>{/if}
	</div>
</section>

<CopyMode bind:text={copying} />

<Dialog
	bind:open={() => !!editing, (open) => !open && (editing = undefined)}
	title={editing && editing.commits.length > 1 ? `Squash ${editing.commits.length} commits` : 'Edit commit message'}
	description={editing
		? editing.commits.length > 1
			? `${editing.commits.at(-1)!.short} and ${editing.commits.length === 2 ? 'the newer commit' : `${editing.commits.length - 1} newer commits`} become one`
			: `${editing.commits[0].short} · ${editing.commits[0].subject}`
		: ''}
>
	{#if editing}
		<!-- svelte-ignore a11y_no_noninteractive_element_interactions (Ctrl+Enter saves from either field.) -->
		<form
			id="reword"
			onsubmit={save}
			onkeydown={(event) => {
				if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) save();
			}}
		>
			<input
				bind:value={editing.summary}
				aria-label="Summary"
				placeholder="Summary"
				spellcheck="true"
				autocomplete="off"
				enterkeyhint="next"
				disabled={!editing.loaded || saving}
				onkeydown={(event) => {
					if (event.key !== 'Enter' || event.ctrlKey || event.metaKey) return;
					event.preventDefault();
					event.currentTarget.form?.querySelector('textarea')?.focus();
				}}
			/>
			<textarea
				bind:value={editing.description}
				aria-label="Description"
				placeholder="Description"
				rows="8"
				spellcheck="true"
				disabled={!editing.loaded || saving}
			></textarea>
			{#if refusal}<p role="alert">{refusal}</p>{/if}
		</form>
	{/if}
	{#snippet actions()}
		<Button disabled={saving} onclick={() => (editing = undefined)}>Cancel</Button>
		<Button type="submit" form="reword" variant="primary" disabled={saving || !editing?.loaded || !editing.summary.trim()}>
			{saving ? 'Saving…' : editing && editing.commits.length > 1 ? 'Squash' : 'Save'}
		</Button>
	{/snippet}
</Dialog>

<style>
	section {
		display: flex;
		flex: 1;
		flex-direction: column;
		min-height: 0;
	}

	header {
		display: flex;
		align-items: center;
		min-height: var(--target-min);
		padding: 0 var(--space-3);
	}

	h2 {
		margin: 0;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		font-weight: 600;
	}

	.list {
		flex: 1;
		overflow: auto;
		padding-bottom: var(--space-3);
	}

	ul {
		margin: 0;
		padding: 0;
		list-style: none;
	}

	button {
		display: flex;
		gap: var(--space-2);
		width: 100%;
		min-height: var(--target-min);
		border: none;
		background: none;
		color: var(--color-fg);
		font: inherit;
		text-align: left;
		white-space: nowrap;
		cursor: pointer;
	}

	button:hover {
		background: var(--hover);
	}

	button:focus-visible {
		outline: var(--border-width) solid var(--status-working);
		outline-offset: calc(-1 * var(--border-width));
	}

	.row {
		display: flex;
		align-items: center;
	}

	.row:hover {
		background: var(--hover);
	}

	.row button:hover {
		background: none;
	}

	form {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}

	input,
	textarea {
		box-sizing: border-box;
		width: 100%;
		padding: var(--space-2);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-sm);
		background: var(--surface-2);
		color: var(--color-fg);
		font: inherit;
		line-height: 1.5;
		resize: vertical;
	}

	input:focus-visible,
	textarea:focus-visible {
		outline: var(--border-width) solid var(--status-working);
	}

	.failure {
		color: var(--git-deleted);
	}

	.commit {
		flex: 1;
		min-width: 0;
		align-items: center;
		gap: var(--space-1);
		padding: var(--space-1) var(--space-3) var(--space-1) var(--space-2);
	}

	.caret {
		display: inline-flex;
		flex: none;
		width: 1em;
		color: var(--muted-fg);
		transition: transform var(--dur-fast);
	}

	.caret.open {
		transform: rotate(90deg);
	}

	.text {
		display: flex;
		flex: 1;
		flex-direction: column;
		min-width: 0;
	}

	.subject,
	.meta {
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.meta {
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.sha {
		font-family: var(--font-mono);
	}

	.file {
		align-items: center;
		padding: 0 var(--space-3) 0 calc(var(--space-2) + var(--space-1) + 1em);
	}

	.letter {
		flex: none;
		width: 1ch;
		color: var(--git);
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		font-weight: 700;
	}

	.deleted .name {
		text-decoration: line-through;
	}

	.name {
		flex: none;
		max-width: 70%;
		overflow: hidden;
		font-family: var(--font-mono);
		font-size: var(--font-size-code);
		text-overflow: ellipsis;
	}

	.dir {
		overflow: hidden;
		color: var(--muted-fg);
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		text-overflow: ellipsis;
	}

	.empty,
	p {
		margin: 0;
		padding: var(--space-2) var(--space-3);
		color: var(--muted-fg);
	}

	.empty {
		padding-left: calc(var(--space-2) + var(--space-1) + 1em);
	}
</style>
