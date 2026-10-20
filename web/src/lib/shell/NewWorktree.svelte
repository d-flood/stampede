<script lang="ts">
	import type { Project } from '#lib/device.svelte.ts';
	import { createWorktree, refs, type Refs } from '#lib/git.ts';
	import Button from '#lib/ui/Button.svelte';
	import { onMount } from 'svelte';

	type Props = { project: Project; current?: string; oncreate(projectId: string): void; oncancel(): void };

	let { project, current, oncreate, oncancel }: Props = $props();

	let listed = $state<Refs>();
	let from = $state('');
	let branch = $state('');
	let error = $state<string>();
	let creating = $state(false);

	onMount(() => {
		refs(project.id).then(
			(loaded) => {
				listed = loaded;
				from = loaded.local.find((name) => name === current) ?? loaded.local[0] ?? loaded.remote[0] ?? '';
			},
			(cause: Error) => (error = cause.message)
		);
	});

	async function create(event: SubmitEvent) {
		event.preventDefault();
		if (creating) return;
		creating = true;
		error = undefined;
		try {
			oncreate(await createWorktree(project.id, branch.trim(), from));
		} catch (cause) {
			error = cause instanceof Error ? cause.message : String(cause);
		} finally {
			creating = false;
		}
	}
</script>

<form onsubmit={create}>
	<label for="worktree-from">From</label>
	<select id="worktree-from" bind:value={from} disabled={!listed || creating}>
		{#if listed}
			<optgroup label="Local branches">
				{#each listed.local as name (name)}<option value={name}>{name}</option>{/each}
			</optgroup>
			{#if listed.remote.length}
				<optgroup label="Remote branches">
					{#each listed.remote as name (name)}<option value={name}>{name}</option>{/each}
				</optgroup>
			{/if}
		{/if}
	</select>
	<label for="worktree-branch">New branch</label>
	<input id="worktree-branch" bind:value={branch} required autocomplete="off" autocapitalize="off" spellcheck="false" disabled={creating} />
	{#if error}<p role="alert">{error}</p>{/if}
	<div class="actions">
		<Button type="button" disabled={creating} onclick={oncancel}>Cancel</Button>
		<Button type="submit" variant="primary" disabled={creating || !from || !branch.trim()}>{creating ? 'Creating…' : 'Create'}</Button>
	</div>
</form>

<style>
	form {
		display: grid;
		gap: var(--space-2);
		min-width: 0;
	}

	label {
		margin-top: var(--space-2);
		font-size: var(--font-size-sm);
	}

	select,
	input {
		box-sizing: border-box;
		width: 100%;
		min-width: 0;
		min-height: var(--target-min);
		padding: var(--space-1) var(--space-2);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--color-bg);
		color: var(--color-fg);
		font: inherit;
	}

	select:focus-visible,
	input:focus-visible {
		outline: var(--border-width) solid var(--status-working);
		outline-offset: var(--border-width);
	}

	p {
		margin: var(--space-2) 0;
		overflow-wrap: anywhere;
	}

	.actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-2);
		margin-top: var(--space-3);
	}
</style>
