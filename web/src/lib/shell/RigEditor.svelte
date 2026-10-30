<script lang="ts">
	import { onMount } from 'svelte';
	import { agentNames, deleteRig, reorderRigs, rigs, saveRig, type Agent } from '#lib/config.svelte.ts';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import Select from '#lib/ui/Select.svelte';
	import TextField from '#lib/ui/TextField.svelte';

	type Model = { value: string; label: string; efforts: string[] };
	type Options = { models: Model[]; modes: { value: string; label: string }[]; settings?: { model?: string; effort?: string } };

	let selected = $state('');
	let name = $state('');
	let agent = $state<Agent>('claude-code');
	let model = $state('');
	let effort = $state('');
	let mode = $state('auto');
	let options = $state<Options>({ models: [], modes: [] });
	let loading = $state(false);
	let saving = $state(false);
	let error = $state('');
	let saved = $state('');
	let confirming = $state(false);
	let deleting = $state(false);
	let deleteError = $state('');
	const busy = $derived(saving || deleting);
	const position = $derived(rigs().findIndex((rig) => rig.name === selected));
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
			if (current === request) {
				options = listed;
				if (rigs().some((rig) => rig.name === selected && rig.agent === agent)) {
					model ||= listed.settings?.model ?? '';
					effort ||= listed.settings?.effort ?? '';
				}
			}
		} catch (failure) {
			if (current === request) error = failure instanceof Error ? failure.message : 'Could not load provider choices.';
		} finally {
			if (current === request) loading = false;
		}
	}

	function choose(value: string) {
		selected = value;
		const rig = rigs().find((rig) => rig.name === value);
		name = rig?.name ?? '';
		agent = rig?.agent ?? 'claude-code';
		model = rig?.model ?? '';
		effort = rig?.effort ?? '';
		mode = rig?.mode ?? defaultMode(agent);
		saved = '';
		void load();
	}

	function provider(value: Agent) {
		agent = value;
		model = '';
		effort = '';
		mode = defaultMode(agent);
		saved = '';
		void load();
	}

	function defaultMode(provider: Agent) {
		return { 'claude-code': 'auto', codex: 'approve-for-me', copilot: 'default' }[provider];
	}

	async function save() {
		saving = true;
		error = '';
		saved = '';
		try {
			await saveRig({ original: selected || undefined, name, agent, model: model || undefined, effort: effort || undefined, mode });
			name = name.trim();
			selected = name;
			saved = 'Rig saved.';
		} catch (failure) {
			error = failure instanceof Error ? failure.message : 'Could not save rig.';
		} finally {
			saving = false;
		}
	}

	async function move(offset: number) {
		const order = rigs().map((rig) => rig.name);
		const destination = position + offset;
		if (position < 0 || destination < 0 || destination >= order.length) return;
		[order[position], order[destination]] = [order[destination], order[position]];
		saving = true;
		error = '';
		saved = '';
		try {
			await reorderRigs(order);
			saved = 'Rig order saved.';
		} catch (failure) {
			error = failure instanceof Error ? failure.message : 'Could not save rig order.';
		} finally {
			saving = false;
		}
	}

	async function remove() {
		deleting = true;
		deleteError = '';
		try {
			await deleteRig(selected);
			confirming = false;
			choose('');
		} catch (failure) {
			deleteError = failure instanceof Error ? failure.message : 'Could not delete rig.';
		} finally {
			deleting = false;
		}
	}
</script>

<div class="rig-editor">
	<div class="picker">
		<label for="editing-rig">Rig</label>
		<Select id="editing-rig" value={selected} disabled={busy} onchange={(event) => choose(event.currentTarget.value)}>
			<option value="">New rig</option>
			{#each rigs() as rig, index (rig.name)}<option value={rig.name}>{index + 1}. {rig.name}</option>{/each}
		</Select>
		{#if selected}
			<div class="order" role="group" aria-label="Rig order">
				<Button variant="ghost" disabled={busy || position <= 0} onclick={() => move(-1)}>Move up</Button>
				<Button variant="ghost" disabled={busy || position === rigs().length - 1} onclick={() => move(1)}>Move down</Button>
				{#if position < 9}<p>Launch with <kbd>Ctrl+Shift+{position + 1}</kbd></p>{/if}
			</div>
		{/if}
	</div>
	<fieldset disabled={busy}>
		<label for="rig-name">Name</label>
		<TextField id="rig-name" bind:value={name} required placeholder="e.g. Reviewer" oninput={() => (saved = '')} />
		<label for="rig-provider">Provider</label>
		<Select id="rig-provider" value={agent} onchange={(event) => provider(event.currentTarget.value as Agent)}>
			{#each Object.entries(agentNames) as [value, label]}<option {value}>{label}</option>{/each}
		</Select>
		<label for="rig-model">Model</label>
		<Select id="rig-model" bind:value={model} disabled={loading || !options.models.length} onchange={() => { effort = ''; saved = ''; }}>
			<option value="">Provider default</option>
			{#if model && !options.models.some((choice) => choice.value === model)}<option value={model}>{model} (configured)</option>{/if}
			{#each options.models as choice (choice.value)}<option value={choice.value}>{choice.label}</option>{/each}
		</Select>
		<label for="rig-effort">Effort level</label>
		<Select id="rig-effort" bind:value={effort} disabled={loading || !efforts.length} onchange={() => (saved = '')}>
			<option value="">Provider default</option>
			{#if effort && !efforts.includes(effort)}<option value={effort}>{effort} (configured)</option>{/if}
			{#each efforts as value}<option {value}>{value}</option>{/each}
		</Select>
		<label for="rig-mode">Permission mode</label>
		<Select id="rig-mode" bind:value={mode} disabled={loading || !options.modes.length} onchange={() => (saved = '')}>
			{#if !options.modes.some((choice) => choice.value === mode)}<option value={mode}>{mode}</option>{/if}
			{#each options.modes as choice (choice.value)}<option value={choice.value}>{choice.label}</option>{/each}
		</Select>
	</fieldset>
	{#if loading}<p role="status">Loading provider choices…</p>{/if}
	{#if error}<p role="alert">{error}</p><Button disabled={loading || busy} onclick={load}>Reload choices</Button>{/if}
	<div class="actions">
		{#if saved}<span role="status">{saved}</span>{/if}
		{#if selected}<Button variant="danger" disabled={busy} onclick={() => { deleteError = ''; confirming = true; }}>Delete rig</Button>{/if}
		<Button variant="primary" disabled={loading || busy || !name.trim() || !options.models.length || !options.modes.length} onclick={save}>{saving ? 'Saving…' : 'Save rig'}</Button>
	</div>
</div>

<Dialog bind:open={confirming} title="Delete rig?" description={`Delete “${selected}”? Existing chats will be kept.`}>
	{#if deleteError}<p role="alert">{deleteError}</p>{/if}
	{#snippet actions()}
		<Button disabled={deleting} onclick={() => (confirming = false)}>Cancel</Button>
		<Button variant="danger" disabled={deleting} onclick={remove}>{deleting ? 'Deleting…' : 'Delete'}</Button>
	{/snippet}
</Dialog>

<style>
	.rig-editor,
	fieldset {
		display: grid;
		gap: var(--space-2);
		min-width: 0;
	}

	.rig-editor {
		container-type: inline-size;
	}

	.picker {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr);
		align-items: center;
		gap: var(--space-2) var(--space-3);
		margin-bottom: var(--space-2);
		padding-bottom: var(--space-4);
		border-bottom: var(--border-width) solid var(--border);
	}

	.picker label {
		margin: 0;
		font-weight: 600;
	}

	.picker .order {
		grid-column: 1 / -1;
		justify-content: flex-start;
		margin: 0;
	}

	.order p {
		margin: 0 0 0 auto;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	kbd {
		padding: 0 var(--space-1);
		border: var(--border-width) solid var(--border-strong);
		border-radius: var(--radius-sm);
		font-family: var(--font-mono);
	}

	@container (min-width: 30rem) {
		fieldset {
			grid-template-columns: 9rem minmax(0, 1fr);
			align-items: center;
			column-gap: var(--space-4);
		}

		fieldset label {
			margin: 0;
		}
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

	p {
		margin: var(--space-2) 0;
		overflow-wrap: anywhere;
	}

	.order,
	.actions {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: flex-end;
		gap: var(--space-2);
		margin-top: var(--space-3);
	}
</style>
