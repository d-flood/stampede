<script lang="ts">
	import { rename, RequestError } from '#lib/files.svelte.ts';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import TextField from '#lib/ui/TextField.svelte';

	type Props = { projectId: string; onrename(from: string, to: string): void };

	let { projectId, onrename }: Props = $props();

	const id = $props.id();
	let open = $state(false);
	let target = $state({ path: '', dir: false });
	let name = $state('');
	let renaming = $state(false);
	let error = $state('');
	const original = $derived(target.path.slice(target.path.lastIndexOf('/') + 1));

	export function start(path: string, dir: boolean) {
		if (renaming) return;
		target = { path, dir };
		name = original;
		error = '';
		open = true;
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		const trimmed = name.trim();
		if (!trimmed || renaming) return;
		if (trimmed === original) {
			open = false;
			return;
		}
		renaming = true;
		try {
			const { path } = await rename(projectId, target.path, trimmed);
			onrename(target.path, path);
			open = false;
		} catch (cause) {
			error = cause instanceof RequestError && cause.status === 409
				? 'A file or directory with that name already exists.'
				: cause instanceof Error ? cause.message : String(cause);
		} finally {
			renaming = false;
		}
	}
</script>

<Dialog bind:open title={target.dir ? 'Rename Directory' : 'Rename File'} description={`Rename “${target.path}”`}>
	<form id={`${id}-form`} onsubmit={submit}>
		<label for={`${id}-name`}>Name</label>
		<TextField
			id={`${id}-name`}
			bind:value={name}
			required
			pattern="[^\/]+"
			autocomplete="off"
			spellcheck="false"
			disabled={renaming}
			{@attach (input) => {
				const dot = target.dir ? -1 : original.lastIndexOf('.');
				input.setSelectionRange(0, dot > 0 ? dot : original.length);
			}}
		/>
		{#if error}<p role="alert">{error}</p>{/if}
	</form>
	{#snippet actions()}
		<Button disabled={renaming} onclick={() => (open = false)}>Cancel</Button>
		<Button type="submit" form={`${id}-form`} variant="primary" disabled={renaming || !name.trim()}>{renaming ? 'Renaming…' : 'Rename'}</Button>
	{/snippet}
</Dialog>

<style>
	label {
		display: block;
		margin-bottom: var(--space-1);
	}
</style>
