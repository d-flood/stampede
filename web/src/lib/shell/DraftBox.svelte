<script lang="ts">
	import MarkdownEditor from '#lib/editor/MarkdownEditor.svelte';
	import Button from '#lib/ui/Button.svelte';

	type Props = {
		body: string;
		saved: boolean;
		label: string;
		/** The selected Source lines a suggestion starts from; absent on the Target side. */
		suggestion?: string;
		onsave(body: string): Promise<unknown>;
		oncancel(): void;
		ondelete(): Promise<unknown>;
	};

	let { body, saved, label, suggestion, onsave, oncancel, ondelete }: Props = $props();

	// svelte-ignore state_referenced_locally (The editor owns the text once mounted; a saved body remounts this box.)
	let text = $state(body);
	let generation = $state(0);
	let busy = $state(false);
	let error = $state<string>();
	const dirty = $derived(text !== body);

	async function run(action: () => Promise<unknown>) {
		if (busy) return;
		busy = true;
		error = undefined;
		try {
			await action();
		} catch (cause) {
			error = cause instanceof Error ? cause.message : String(cause);
		} finally {
			busy = false;
		}
	}

	function save() {
		if (text.trim() && (dirty || !saved)) run(() => onsave(text));
	}

	function cancel() {
		if (!saved) return oncancel();
		text = body;
		generation++;
	}

	function suggest() {
		const fence = suggestion?.includes('```') ? '````' : '```';
		text = `${text}${text && !text.endsWith('\n') ? '\n' : ''}${fence}suggestion\n${suggestion}\n${fence}\n`;
		generation++;
	}
</script>

<div class="draft">
	{#key generation}
		<MarkdownEditor bind:text {label} autofocus={!saved || generation > 0} onsave={save} />
	{/key}
	<div class="actions">
		{#if suggestion !== undefined}
			<Button variant="ghost" disabled={busy} onclick={suggest}>Suggest a change</Button>
		{/if}
		{#if error}<span class="error" role="alert">{error}</span>{/if}
		<span class="gap"></span>
		{#if saved}
			<Button variant="danger" disabled={busy} onclick={() => run(ondelete)}>Delete</Button>
		{/if}
		{#if !saved || dirty}
			<Button disabled={busy} onclick={cancel}>Cancel</Button>
			<Button variant="primary" disabled={busy || !text.trim()} onclick={save}>Save</Button>
		{/if}
	</div>
</div>

<style>
	.draft {
		display: grid;
		gap: var(--space-2);
		max-width: 60rem;
		font-family: var(--font-ui);
	}

	.draft :global(.cm-editor) {
		min-height: 3lh;
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
	}

	.draft :global(.cm-editor.cm-focused) {
		border-color: var(--status-working);
	}

	.draft :global(.cm-gutters) {
		display: none;
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
	}

	.gap {
		flex: 1;
	}

	.error {
		color: var(--git-deleted);
		font-size: var(--font-size-sm);
		overflow-wrap: anywhere;
	}
</style>
