<script lang="ts">
	import type { Project } from '#lib/device.svelte.ts';
	import { createWorktree, refs, type Refs } from '#lib/git.ts';
	import Button from '#lib/ui/Button.svelte';
	import Select from '#lib/ui/Select.svelte';
	import TextField from '#lib/ui/TextField.svelte';
	import { onMount } from 'svelte';

	type Props = { project: Project; current?: string; oncreate(projectId: string): void; oncancel(): void };

	let { project, current, oncreate, oncancel }: Props = $props();

	let listed = $state<Refs>();
	let from = $state('');
	let branch = $state('');
	let error = $state<string>();
	let creating = $state(false);
	let fresh = $derived(!!branch.trim());

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
			oncreate(await createWorktree(project.id, from, branch.trim() || undefined));
		} catch (cause) {
			error = cause instanceof Error ? cause.message : String(cause);
		} finally {
			creating = false;
		}
	}
</script>

<form onsubmit={create}>
	<label for="worktree-from">From</label>
	<Select id="worktree-from" bind:value={from} disabled={!listed || creating}>
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
	</Select>
	<label for="worktree-branch">New branch</label>
	<TextField
		id="worktree-branch"
		bind:value={branch}
		placeholder={from && `Leave blank to check out ${from}`}
		autocomplete="off"
		autocapitalize="off"
		spellcheck="false"
		disabled={creating}
	/>
	{#if error}<p role="alert">{error}</p>{/if}
	<div class="actions">
		<Button type="button" disabled={creating} onclick={oncancel}>Cancel</Button>
		<Button type="submit" variant="primary" disabled={creating || !from}>
			{#if fresh}{creating ? 'Creating…' : 'Create'}{:else}{creating ? 'Opening…' : 'Open'}{/if}
		</Button>
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
