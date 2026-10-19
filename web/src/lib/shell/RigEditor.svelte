<script lang="ts">
	import { onMount } from 'svelte';
	import { agentNames, rigs, saveRig, type Agent } from '#lib/config.svelte.ts';
	import Button from '#lib/ui/Button.svelte';

	type Model = { value: string; label: string; efforts: string[] };
	type Options = { models: Model[]; modes: { value: string; label: string }[] };

	let selected = $state('');
	let agent = $state<Agent>('claude-code');
	let model = $state('');
	let effort = $state('');
	let mode = $state('auto');
	let options = $state<Options>({ models: [], modes: [] });
	let loading = $state(false);
	let saving = $state(false);
	let error = $state('');
	let saved = $state(false);
	let request = 0;
	const efforts = $derived(options.models.find((choice) => choice.value === model)?.efforts ?? []);

	onMount(() => { void load(); });

	async function load() {
		const current = ++request;
		loading = true;
		error = '';
		options = { models: [], modes: [] };
		try {
			const query = new URLSearchParams({ agent });
			if (selected) query.set('rig', selected);
			const response = await fetch(`/api/rigs/options?${query}`);
			if (!response.ok) throw new Error(await response.text());
			const listed: Options = await response.json();
			if (!listed.models.length) throw new Error('Provider reported no models.');
			if (current === request) options = listed;
		} catch (failure) {
			if (current === request) error = failure instanceof Error ? failure.message : 'Could not load provider choices.';
		} finally {
			if (current === request) loading = false;
		}
	}

	function choose(name: string) {
		selected = name;
		const rig = rigs().find((rig) => rig.name === name);
		agent = rig?.agent ?? 'claude-code';
		model = rig?.model ?? '';
		effort = rig?.effort ?? '';
		mode = rig?.mode ?? (agent === 'claude-code' ? 'auto' : 'approve-for-me');
		saved = false;
		void load();
	}

	function provider(value: Agent) {
		agent = value;
		model = '';
		effort = '';
		mode = agent === 'claude-code' ? 'auto' : 'approve-for-me';
		saved = false;
		void load();
	}

	async function save() {
		saving = true;
		error = '';
		saved = false;
		try {
			await saveRig({ original: selected || undefined, agent, model: model || undefined, effort: effort || undefined, mode });
			if (!selected) selected = rigs().at(-1)!.name;
			saved = true;
		} catch (failure) {
			error = failure instanceof Error ? failure.message : 'Could not save rig.';
		} finally {
			saving = false;
		}
	}
</script>

<div class="rig-editor">
	<label for="editing-rig">Rig</label>
	<select id="editing-rig" value={selected} disabled={saving} onchange={(event) => choose(event.currentTarget.value)}>
		<option value="">New rig</option>
		{#each rigs() as rig (rig.name)}<option value={rig.name}>{rig.name}</option>{/each}
	</select>
	<fieldset disabled={saving}>
		<label for="rig-provider">Provider</label>
		<select id="rig-provider" value={agent} onchange={(event) => provider(event.currentTarget.value as Agent)}>
			{#each Object.entries(agentNames) as [value, label]}<option {value}>{label}</option>{/each}
		</select>
		<label for="rig-model">Model</label>
		<select id="rig-model" bind:value={model} disabled={loading || !options.models.length} onchange={() => { effort = ''; saved = false; }}>
			<option value="">Provider default</option>
			{#if model && !options.models.some((choice) => choice.value === model)}<option value={model}>{model} (configured)</option>{/if}
			{#each options.models as choice (choice.value)}<option value={choice.value}>{choice.label}</option>{/each}
		</select>
		<label for="rig-effort">Effort level</label>
		<select id="rig-effort" bind:value={effort} disabled={loading || !efforts.length} onchange={() => (saved = false)}>
			<option value="">Provider default</option>
			{#if effort && !efforts.includes(effort)}<option value={effort}>{effort} (configured)</option>{/if}
			{#each efforts as value}<option {value}>{value}</option>{/each}
		</select>
		<label for="rig-mode">Permission mode</label>
		<select id="rig-mode" bind:value={mode} disabled={loading || !options.modes.length} onchange={() => (saved = false)}>
			{#if !options.modes.some((choice) => choice.value === mode)}<option value={mode}>{mode}</option>{/if}
			{#each options.modes as choice (choice.value)}<option value={choice.value}>{choice.label}</option>{/each}
		</select>
	</fieldset>
	{#if loading}<p role="status">Loading provider choices…</p>{/if}
	{#if error}<p role="alert">{error}</p><Button disabled={loading || saving} onclick={load}>Reload choices</Button>{/if}
	<div class="actions">
		{#if saved}<span role="status">Rig saved.</span>{/if}
		<Button variant="primary" disabled={loading || saving || !options.models.length || !options.modes.length} onclick={save}>{saving ? 'Saving…' : 'Save rig'}</Button>
	</div>
</div>

<style>
	.rig-editor,
	fieldset {
		display: grid;
		gap: var(--space-2);
		min-width: 0;
	}

	fieldset {
		margin: 0;
		padding: 0;
		border: 0;
	}

	label {
		margin-top: var(--space-2);
		font-size: var(--font-size-sm);
	}

	select {
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

	select:focus-visible {
		outline: var(--border-width) solid var(--status-working);
		outline-offset: var(--border-width);
	}

	p {
		margin: var(--space-2) 0;
		overflow-wrap: anywhere;
	}

	.actions {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		gap: var(--space-2);
		margin-top: var(--space-3);
	}
</style>
