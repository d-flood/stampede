<script lang="ts">
	import { clips, deleteClip, reorderClips, saveClip } from '#lib/config.svelte.ts';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';

	let { draft = '', ondone }: { draft?: string; ondone?: () => void } = $props();

	let selected = $state('');
	let name = $state('');
	let text = $derived(draft);
	let saving = $state(false);
	let error = $state('');
	let saved = $state('');
	let confirming = $state(false);
	let deleting = $state(false);
	let deleteError = $state('');
	const busy = $derived(saving || deleting);
	const position = $derived(clips().findIndex((clip) => clip.name === selected));

	function choose(value: string) {
		selected = value;
		const clip = clips().find((clip) => clip.name === value);
		name = clip?.name ?? '';
		text = clip?.text ?? '';
		error = '';
		saved = '';
	}

	async function save() {
		saving = true;
		error = '';
		saved = '';
		try {
			await saveClip({ original: selected || undefined, name, text });
			if (ondone) return ondone();
			name = name.trim();
			selected = name;
			saved = 'Clip saved.';
		} catch (failure) {
			error = failure instanceof Error ? failure.message : 'Could not save clip.';
		} finally {
			saving = false;
		}
	}

	async function move(offset: number) {
		const order = clips().map((clip) => clip.name);
		const destination = position + offset;
		if (position < 0 || destination < 0 || destination >= order.length) return;
		[order[position], order[destination]] = [order[destination], order[position]];
		saving = true;
		error = '';
		saved = '';
		try {
			await reorderClips(order);
			saved = 'Clip order saved.';
		} catch (failure) {
			error = failure instanceof Error ? failure.message : 'Could not save clip order.';
		} finally {
			saving = false;
		}
	}

	async function remove() {
		deleting = true;
		deleteError = '';
		try {
			await deleteClip(selected);
			confirming = false;
			choose('');
		} catch (failure) {
			deleteError = failure instanceof Error ? failure.message : 'Could not delete clip.';
		} finally {
			deleting = false;
		}
	}
</script>

<div class="clip-editor">
	{#if !ondone}
		<div class="picker">
			<label for="editing-clip">Clip</label>
			<select id="editing-clip" value={selected} disabled={busy} onchange={(event) => choose(event.currentTarget.value)}>
				<option value="">New clip</option>
				{#each clips() as clip, index (clip.name)}<option value={clip.name}>{index + 1}. {clip.name}</option>{/each}
			</select>
			{#if selected}
				<div class="order" role="group" aria-label="Clip order">
					<Button variant="ghost" disabled={busy || position <= 0} onclick={() => move(-1)}>Move up</Button>
					<Button variant="ghost" disabled={busy || position === clips().length - 1} onclick={() => move(1)}>Move down</Button>
				</div>
			{/if}
		</div>
	{/if}
	<fieldset disabled={busy}>
		<label for="clip-name">Clip name</label>
		<input id="clip-name" bind:value={name} required placeholder="e.g. verify" autocapitalize="off" autocomplete="off" spellcheck="false" aria-describedby="clip-name-help" oninput={() => (saved = '')} />
		<p id="clip-name-help">Type ;{name.trim() || 'name'} in any Composer to insert it.</p>
		<label for="clip-text">Clip text</label>
		<textarea id="clip-text" bind:value={text} rows="6" oninput={() => (saved = '')}></textarea>
	</fieldset>
	{#if error}<p role="alert">{error}</p>{/if}
	<div class="actions">
		{#if saved}<span role="status">{saved}</span>{/if}
		{#if selected}<Button variant="danger" disabled={busy} onclick={() => { deleteError = ''; confirming = true; }}>Delete clip</Button>{/if}
		{#if ondone}<Button disabled={busy} onclick={ondone}>Cancel</Button>{/if}
		<Button variant="primary" disabled={busy || !name.trim() || !text.trim()} onclick={save}>{saving ? 'Saving…' : 'Save clip'}</Button>
	</div>
</div>

<Dialog bind:open={confirming} title="Delete clip?" description={`Delete “${selected}”?`}>
	{#if deleteError}<p role="alert">{deleteError}</p>{/if}
	{#snippet actions()}
		<Button disabled={deleting} onclick={() => (confirming = false)}>Cancel</Button>
		<Button variant="danger" disabled={deleting} onclick={remove}>{deleting ? 'Deleting…' : 'Delete'}</Button>
	{/snippet}
</Dialog>

<style>
	.clip-editor,
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

	.clip-editor {
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

	@container (min-width: 30rem) {
		fieldset {
			grid-template-columns: 9rem minmax(0, 1fr);
			align-items: center;
			column-gap: var(--space-4);
		}

		fieldset label {
			margin: 0;
		}

		fieldset p {
			grid-column: 2;
		}

		label[for='clip-text'] {
			align-self: start;
			margin-top: var(--space-2);
		}
	}

	label {
		margin-top: var(--space-2);
		font-size: var(--font-size-sm);
	}

	select,
	input,
	textarea {
		box-sizing: border-box;
		width: 100%;
		min-width: 0;
		min-height: var(--target-min);
		padding: var(--space-1) var(--space-2);
		border: var(--border-width) solid var(--border-strong);
		border-radius: var(--radius-md);
		background: var(--surface-1);
		color: var(--color-fg);
		font: inherit;
	}

	textarea {
		resize: vertical;
	}

	select:focus-visible,
	input:focus-visible,
	textarea:focus-visible {
		outline: 2px solid color-mix(in oklab, var(--status-working) 70%, transparent);
		outline-offset: 1px;
	}

	p {
		margin: 0;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		overflow-wrap: anywhere;
	}

	[role='alert'] {
		color: var(--ansi-1);
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
