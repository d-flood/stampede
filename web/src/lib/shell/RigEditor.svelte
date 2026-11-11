<script lang="ts">
	import { logos } from '#lib/chat/sessions.ts';
	import { agentNames, deleteRig, modes, reorderRigs, rigs, saveRig, type Agent, type Rig } from '#lib/config.svelte.ts';
	import { form } from '#lib/form.ts';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import Kbd from '#lib/ui/Kbd.svelte';
	import Select from '#lib/ui/Select.svelte';
	import TextField from '#lib/ui/TextField.svelte';
	import CaretLeft from '~icons/ph/caret-left';
	import CaretRight from '~icons/ph/caret-right';
	import DotsSixVertical from '~icons/ph/dots-six-vertical';
	import Plus from '~icons/ph/plus';

	type Model = { value: string; label: string; efforts: string[]; speeds: { value: string; label: string; detail?: string }[]; ultracode: boolean };
	type Options = { models: Model[]; modes: { value: string; label: string }[]; settings?: { model?: string; effort?: string } };

	let { editing = $bindable(false) }: { editing?: boolean } = $props();

	let selected = $state<string>();
	let name = $state('');
	let agent = $state<Agent>('claude-code');
	let model = $state('');
	let effort = $state('');
	let speed = $state('');
	let ultracode = $state('');
	let mode = $state('auto');
	let options = $state<Options>({ models: [], modes: [] });
	let loading = $state(false);
	let saving = $state(false);
	let error = $state('');
	let saved = $state('');
	let ordering = $state(false);
	let ordered = $state('');
	let orderError = $state('');
	let confirming = $state(false);
	let deleting = $state(false);
	let deleteError = $state('');
	let dragging = $state<string>();
	let over = $state<number>();
	const phone = $derived(form() === 'phone');
	const busy = $derived(saving || deleting || ordering);
	const position = $derived(rigs().findIndex((rig) => rig.name === selected));
	const from = $derived(rigs().findIndex((rig) => rig.name === dragging));
	let request = 0;
	const listed = $derived(options.models.find((choice) => choice.value === model));
	const efforts = $derived(listed?.efforts ?? []);
	const speeds = $derived(listed?.speeds ?? []);

	$effect(() => {
		editing = phone && selected !== undefined;
		return () => (editing = false);
	});

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
		speed = rig?.speed ?? '';
		ultracode = rig?.ultracode ?? '';
		mode = rig?.mode ?? defaultMode(agent);
		saved = '';
		void load();
	}

	function close() {
		request++;
		selected = undefined;
		loading = false;
		error = '';
		saved = '';
	}

	function provider(value: Agent) {
		agent = value;
		model = '';
		effort = '';
		speed = '';
		ultracode = '';
		mode = defaultMode(agent);
		saved = '';
		void load();
	}

	function defaultMode(provider: Agent) {
		return { 'claude-code': 'auto', codex: 'approve-for-me', copilot: 'default' }[provider];
	}

	function summary(rig: Rig) {
		const value = rig.mode ?? defaultMode(rig.agent);
		const label = modes(rig.agent).find((choice) => choice.value === value)?.label ?? value;
		return [rig.model ?? 'Default model', rig.effort, label, rig.speed !== 'standard' && rig.speed !== 'default' && rig.speed, rig.ultracode === 'on' && 'Ultracode on'].filter(Boolean).join(', ');
	}

	async function save() {
		saving = true;
		error = '';
		saved = '';
		ordered = '';
		try {
			await saveRig({ original: selected || undefined, name, agent, model: model || undefined, effort: effort || undefined, speed: speed || undefined, ultracode: ultracode || undefined, mode });
			name = name.trim();
			selected = name;
			saved = 'Rig saved.';
		} catch (failure) {
			error = failure instanceof Error ? failure.message : 'Could not save rig.';
		} finally {
			saving = false;
		}
	}

	async function reorder(order: string[]) {
		ordering = true;
		orderError = '';
		ordered = '';
		saved = '';
		try {
			await reorderRigs(order);
			ordered = 'Rig order saved.';
		} catch (failure) {
			orderError = failure instanceof Error ? failure.message : 'Could not save rig order.';
		} finally {
			ordering = false;
		}
	}

	function move(offset: number) {
		const order = rigs().map((rig) => rig.name);
		const destination = position + offset;
		if (position < 0 || destination < 0 || destination >= order.length) return;
		[order[position], order[destination]] = [order[destination], order[position]];
		void reorder(order);
	}

	function drop(index: number) {
		const order = rigs().map((rig) => rig.name);
		const source = from;
		dragging = over = undefined;
		if (source < 0 || source === index) return;
		order.splice(index, 0, ...order.splice(source, 1));
		void reorder(order);
	}

	async function remove() {
		deleting = true;
		deleteError = '';
		try {
			await deleteRig(selected ?? '');
			confirming = false;
			close();
		} catch (failure) {
			deleteError = failure instanceof Error ? failure.message : 'Could not delete rig.';
		} finally {
			deleting = false;
		}
	}
</script>

{#snippet editor()}
	<div class="editor">
		<fieldset disabled={busy}>
			<label for="rig-name">Name</label>
			<TextField id="rig-name" bind:value={name} required placeholder="e.g. Reviewer" oninput={() => (saved = '')} />
			<label for="rig-provider">Provider</label>
			<Select id="rig-provider" value={agent} onchange={(event) => provider(event.currentTarget.value as Agent)}>
				{#each Object.entries(agentNames) as [value, label]}<option {value}>{label}</option>{/each}
			</Select>
			<label for="rig-model">Model</label>
			<Select id="rig-model" bind:value={model} disabled={loading || !options.models.length} onchange={() => { effort = ''; speed = ''; ultracode = ''; saved = ''; }}>
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
			{#if speeds.length || speed}
				<label for="rig-speed">Speed</label>
				<Select id="rig-speed" bind:value={speed} disabled={loading} onchange={() => (saved = '')}>
					<option value="">Provider default</option>
					{#if speed && !speeds.some((choice) => choice.value === speed)}<option value={speed}>{speed} (configured)</option>{/if}
					{#each speeds as choice (choice.value)}<option value={choice.value}>{choice.label}{choice.detail ? ` (${choice.detail})` : ''}</option>{/each}
				</Select>
			{/if}
			{#if listed?.ultracode || ultracode}
				<label for="rig-ultracode">Ultracode</label>
				<Select id="rig-ultracode" bind:value={ultracode} disabled={loading} onchange={() => (saved = '')}>
					<option value="">Provider default</option>
					<option value="off">off</option>
					<option value="on">on</option>
				</Select>
			{/if}
			<label for="rig-mode">Permission mode</label>
			<Select id="rig-mode" bind:value={mode} disabled={loading || !options.modes.length} onchange={() => (saved = '')}>
				{#if !options.modes.some((choice) => choice.value === mode)}<option value={mode}>{mode}</option>{/if}
				{#each options.modes as choice (choice.value)}<option value={choice.value}>{choice.label}</option>{/each}
			</Select>
		</fieldset>
		{#if loading}<p role="status">Loading provider choices…</p>{/if}
		{#if error}<p role="alert">{error}</p><Button disabled={loading || busy} onclick={load}>Reload choices</Button>{/if}
		<div class="actions">
			{#if selected}
				<div class="order" role="group" aria-label="Rig order">
					<Button variant="ghost" disabled={busy || position <= 0} onclick={() => move(-1)}>Move up</Button>
					<Button variant="ghost" disabled={busy || position === rigs().length - 1} onclick={() => move(1)}>Move down</Button>
				</div>
			{/if}
			{#if saved}<span role="status">{saved}</span>{/if}
			{#if selected}<Button variant="danger" disabled={busy} onclick={() => { deleteError = ''; confirming = true; }}>Delete rig</Button>{/if}
			<Button variant="primary" disabled={loading || busy || !name.trim() || !options.models.length || !options.modes.length} onclick={save}>{saving ? 'Saving…' : 'Save rig'}</Button>
		</div>
	</div>
{/snippet}

{#snippet feedback()}
	{#if ordered}<p role="status">{ordered}</p>{/if}
	{#if orderError}<p role="alert">{orderError}</p>{/if}
{/snippet}

{#if phone && selected !== undefined}
	<div class="page">
		<Button variant="link" aria-label="All Rigs" onclick={close}><CaretLeft /> Rigs</Button>
		<h3>{selected || 'New Rig'}</h3>
		{@render feedback()}
		{@render editor()}
	</div>
{:else}
	<ol class="rigs" class:phone class:empty={!rigs().length && selected !== ''} aria-label="Rigs">
		{#each rigs() as rig, index (rig.name)}
			{@const Logo = logos[rig.agent]}
			<li
				class:open={selected === rig.name}
				class:dragged={dragging === rig.name}
				class:before={over === index && from > index}
				class:after={over === index && from >= 0 && from < index}
				ondragover={(event) => { if (dragging === undefined) return; event.preventDefault(); over = index; }}
				ondrop={(event) => { event.preventDefault(); drop(index); }}
			>
				<div class="row">
					{#if !phone}
						<span
							class="grip"
							draggable={!busy}
							title="Drag to reorder"
							aria-hidden="true"
							ondragstart={(event) => {
								dragging = rig.name;
								event.dataTransfer?.setData('text/plain', rig.name);
								const row = event.currentTarget.closest('li');
								if (row) event.dataTransfer?.setDragImage(row, 16, 16);
							}}
							ondragend={() => (dragging = over = undefined)}
						><DotsSixVertical /></span>
					{/if}
					<button class="item" aria-expanded={selected === rig.name} aria-labelledby="rig-{index}-name" aria-describedby="rig-{index}-detail" disabled={busy} onclick={() => (selected === rig.name ? close() : choose(rig.name))}>
						<span class="logo"><Logo /></span>
						<span class="text">
							<span class="name" id="rig-{index}-name">{rig.name}</span>
							<span class="detail" id="rig-{index}-detail">{summary(rig)}{#if index < 9}<span class="sr">, Ctrl+Shift+{index + 1}</span>{/if}</span>
						</span>
						{#if phone}<CaretRight />{:else if index < 9}<Kbd aria-hidden="true">Ctrl+Shift+{index + 1}</Kbd>{/if}
					</button>
				</div>
				{#if selected === rig.name && !phone}{@render editor()}{/if}
			</li>
		{/each}
		{#if selected === '' && !phone}
			{@const Logo = logos[agent]}
			<li class="open">
				<div class="row">
					<span class="grip"></span>
					<button class="item" aria-expanded="true" onclick={close}>
						<span class="logo"><Logo /></span>
						<span class="text"><span class="name">New Rig</span><span class="detail">Not saved yet</span></span>
					</button>
				</div>
				{@render editor()}
			</li>
		{/if}
	</ol>
	<div class="footer">
		<Button variant="ghost" disabled={busy || selected === ''} onclick={() => choose('')}><Plus /> New Rig</Button>
		{@render feedback()}
	</div>
{/if}

<Dialog bind:open={confirming} title="Delete rig?" description={`Delete “${selected}”? Existing chats will be kept.`}>
	{#if deleteError}<p role="alert">{deleteError}</p>{/if}
	{#snippet actions()}
		<Button disabled={deleting} onclick={() => (confirming = false)}>Cancel</Button>
		<Button variant="danger" disabled={deleting} onclick={remove}>{deleting ? 'Deleting…' : 'Delete'}</Button>
	{/snippet}
</Dialog>

<style>
	.rigs {
		margin: 0;
		padding: 0;
		overflow: hidden;
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--color-bg);
		list-style: none;
	}

	.empty {
		display: none;
	}

	li + li {
		border-top: var(--border-width) solid var(--border);
	}

	li.open {
		background: var(--surface-1);
	}

	li.dragged {
		opacity: 0.5;
	}

	li.before {
		box-shadow: inset 0 2px 0 var(--accent);
	}

	li.after {
		box-shadow: inset 0 -2px 0 var(--accent);
	}

	.row {
		display: flex;
		align-items: stretch;
	}

	.grip {
		display: grid;
		flex: none;
		place-items: center;
		width: 1.75rem;
		padding-left: var(--space-1);
		color: var(--muted-fg);
		cursor: grab;
	}

	.item {
		display: flex;
		flex: 1;
		align-items: center;
		gap: var(--space-3);
		min-width: 0;
		min-height: calc(var(--target-min) + var(--space-2));
		padding: var(--space-2) var(--space-3) var(--space-2) var(--space-1);
		border: none;
		background: none;
		color: var(--color-fg);
		font: inherit;
		text-align: start;
		cursor: pointer;
	}

	.item:disabled {
		cursor: default;
	}

	@media (hover: hover) {
		.row:hover:has(.item:enabled) {
			background: var(--hover);
		}
	}

	.item:focus-visible {
		outline: var(--focus-ring);
		outline-offset: var(--focus-inset);
	}

	.logo {
		display: inline-flex;
		flex: none;
		font-size: var(--font-size-lg);
	}

	.text {
		display: grid;
		flex: 1;
		min-width: 0;
	}

	.name {
		font-weight: 600;
	}

	.detail {
		overflow: hidden;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.sr {
		position: absolute;
		width: 1px;
		height: 1px;
		overflow: hidden;
		clip-path: inset(50%);
		white-space: nowrap;
	}


	.phone .item {
		padding-left: var(--space-3);
	}

	.phone .item > :global(svg) {
		flex: none;
		color: var(--muted-fg);
	}

	.editor,
	fieldset {
		display: grid;
		gap: var(--space-2);
		min-width: 0;
	}

	.editor {
		container-type: inline-size;
		padding: 0 var(--space-4) var(--space-4) calc(1.75rem + var(--space-1) + var(--space-1));
	}

	.page .editor {
		padding: 0;
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

	[role='alert'] {
		color: var(--ansi-1);
	}

	.actions,
	.footer {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
	}

	.actions {
		justify-content: flex-end;
		margin-top: var(--space-3);
	}

	.order {
		display: flex;
		gap: var(--space-1);
		margin-right: auto;
	}

	.footer {
		margin-top: var(--space-2);
	}

	.footer p {
		margin: 0;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.footer [role='alert'] {
		color: var(--ansi-1);
	}

	h3 {
		margin: var(--space-1) 0 var(--space-4);
		font-size: var(--font-size-lg);
		overflow-wrap: anywhere;
	}
</style>
