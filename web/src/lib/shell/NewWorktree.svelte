<script lang="ts">
	import { hostName } from '#lib/config.svelte.ts';
	import type { Project } from '#lib/device.svelte.ts';
	import { createWorktree, refs, type CloudStep, type Refs } from '#lib/git.ts';
	import { projectUrl } from '#lib/host.ts';
	import Button from '#lib/ui/Button.svelte';
	import Segmented from '#lib/ui/Segmented.svelte';
	import Select from '#lib/ui/Select.svelte';
	import TextField from '#lib/ui/TextField.svelte';
	import { onMount } from 'svelte';
	import Cloud from '~icons/ph/cloud';

	type Props = { project: Project; current?: string; creating?: boolean; oncreate(projectId: string): void; oncancel(): void };

	let { project, current, creating = $bindable(false), oncreate, oncancel }: Props = $props();

	const steps: Record<CloudStep, string> = {
		create: 'Creating Cloud Host…',
		install: 'Installing Stampede…',
		repository: 'Copying repository…',
		carry: 'Copying files…',
		toolkit: 'Adding logins…'
	};

	let listed = $state<Refs>();
	let from = $state('');
	let branch = $state('');
	let error = $state<string>();
	let step = $state<CloudStep>();
	let host = $state<'home' | 'cloud'>('home');
	let cli = $state<'missing' | 'signed-out' | 'ready'>();
	let setup = $state<{ from: string; present: boolean; error?: string }>();
	let fresh = $derived(!!branch.trim());
	let checked = $derived(setup?.from === from);
	let unavailable = $derived(cli !== 'ready' || (setup && !setup.present));

	onMount(() => {
		refs(project.id).then(
			(loaded) => {
				listed = loaded;
				from = loaded.local.find((name) => name === current) ?? loaded.local[0] ?? loaded.remote[0] ?? '';
			},
			(cause: Error) => (error = cause.message)
		);
		fetch('/api/cloud').then(async (response) => {
			if (response.ok) cli = (await response.json()).cli;
		});
	});

	$effect(() => {
		if (!from) return;
		const asked = from;
		const controller = new AbortController();
		fetch(projectUrl(project.id, `cloud-setup?${new URLSearchParams({ from: asked })}`), { signal: controller.signal })
			.then(async (response) => {
				if (!response.ok) return;
				const result = { from: asked, ...(await response.json()) };
				setup = result;
				if (!result.present) host = 'home';
			})
			.catch(() => {});
		return () => controller.abort();
	});

	async function create(event: SubmitEvent) {
		event.preventDefault();
		if (creating) return;
		creating = true;
		error = undefined;
		try {
			oncreate(await createWorktree(project.id, from, branch.trim() || undefined, host === 'cloud' ? (next) => (step = next) : undefined));
		} catch (cause) {
			error = cause instanceof Error ? cause.message : String(cause);
		} finally {
			creating = false;
			step = undefined;
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
	<span class="label" id="worktree-host">Host</span>
	<Segmented
		aria-labelledby="worktree-host"
		aria-describedby="worktree-host-hint"
		options={[
			{ value: 'home', label: hostName() ?? 'Stampede' },
			{ value: 'cloud', label: 'Cloud Host', icon: Cloud, disabled: unavailable }
		]}
		bind:value={host}
		disabled={creating}
	/>
	{#if cli === 'missing'}
		<p class="hint" id="worktree-host-hint">Install the sprite CLI and sign in to use Cloud Hosts.</p>
	{:else if cli === 'signed-out'}
		<p class="hint" id="worktree-host-hint">The sprite CLI is signed out. Run <code>sprite login</code>.</p>
	{:else if setup && !setup.present}
		<p class="hint" id="worktree-host-hint">
			{#if setup.error?.startsWith(`${setup.from} has no Cloud Setup`)}{setup.from} has no Cloud Setup. Type <code>/cloud-setup</code> in a Chat to draft one.{:else}{setup.error}{/if}
		</p>
	{:else if host === 'cloud'}
		<p class="hint" id="worktree-host-hint">Created for this worktree and destroyed with it. Runs the Cloud Setup from {from}.</p>
	{/if}
	{#if error}<p role="alert">{error}</p>{/if}
	<div class="actions">
		<Button type="button" disabled={creating} onclick={oncancel}>Cancel</Button>
		<Button type="submit" variant="primary" busy={step && true} disabled={creating || !from || (host === 'cloud' && !checked)}>
			{#if step}{steps[step]}{:else if fresh}{creating ? 'Creating…' : 'Create'}{:else}{creating ? 'Opening…' : 'Open'}{/if}
		</Button>
	</div>
</form>

<style>
	form {
		display: grid;
		gap: var(--space-2);
		min-width: 0;
	}

	label,
	.label {
		margin-top: var(--space-2);
		font-size: var(--font-size-sm);
	}

	p {
		margin: var(--space-2) 0;
		overflow-wrap: anywhere;
	}

	.hint {
		margin: 0;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	code {
		font-family: var(--font-mono);
		font-size: inherit;
		white-space: nowrap;
	}

	.actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-2);
		margin-top: var(--space-3);
	}
</style>
