<script lang="ts">
	import type { Device } from '#lib/device.ts';
	import { commitFiles, letters, log, split, type Change, type Commit } from '#lib/git.ts';
	import { onMount } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import CaretRight from '~icons/ph/caret-right';

	type Props = { projectId: string; device: Device; onopen(change: Change, commit: Commit): void };

	let { projectId, device, onopen }: Props = $props();

	let commits = $state.raw<Commit[]>();
	let cursor = $state<string>();
	let files = $state<Record<string, Change[]>>({});
	const expanded = new SvelteSet<string>();
	let refreshes = 0;
	let requested: string | undefined;

	onMount(() => {
		refresh();
		return device.listen({
			message(message) {
				if (message.type === 'git.changed' && message.projectId === projectId) refresh();
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
</script>

<section aria-label="History">
	<header><h2>History</h2></header>
	<div class="list" onscroll={scrolled}>
		<ul aria-label="Commits">
			{#each commits ?? [] as commit (commit.sha)}
				{@const open = expanded.has(commit.sha)}
				<li>
					<button class="commit" aria-expanded={open} title={commit.subject} onclick={() => toggle(commit.sha)}>
						<span class="caret" class:open><CaretRight /></span>
						<span class="text">
							<span class="subject">{commit.subject}</span>
							<span class="meta"><span class="sha">{commit.short}</span> · {commit.author} · {commit.date}</span>
						</span>
					</button>
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

	.commit {
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
		text-overflow: ellipsis;
	}

	.dir {
		overflow: hidden;
		color: var(--muted-fg);
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
