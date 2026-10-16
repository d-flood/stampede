<script lang="ts">
	import Button from '#lib/ui/Button.svelte';

	type Props = {
		id: string;
		disabled: boolean;
		insert?: boolean;
		placeholder?: string;
		onsend(text: string, enter: boolean): boolean;
	};

	let { id, disabled, insert = true, placeholder, onsend }: Props = $props();

	const key = $derived(`composer:${id}`);
	let text = $derived(localStorage.getItem(key) ?? '');
	let box: HTMLTextAreaElement;

	function keep() {
		if (text) localStorage.setItem(key, text);
		else localStorage.removeItem(key);
	}

	function submit(enter: boolean) {
		if (!onsend(text, enter)) return;
		text = '';
		keep();
		box.focus();
	}

	export function focus() {
		box.focus();
	}
</script>

<div class="composer">
	<textarea
		bind:this={box}
		bind:value={text}
		oninput={keep}
		aria-label="Composer"
		{placeholder}
		rows="3"
		autocorrect="on"
		autocapitalize="sentences"
		spellcheck="true"
	></textarea>
	<div class="actions">
		{#if insert}
			<Button disabled={!text || disabled} onmousedown={(event) => event.preventDefault()} onclick={() => submit(false)}>
				Insert
			</Button>
		{/if}
		<Button
			variant="primary"
			disabled={!text || disabled}
			onmousedown={(event) => event.preventDefault()}
			onclick={() => submit(true)}
		>
			Send
		</Button>
	</div>
</div>

<style>
	.composer {
		display: flex;
		flex: none;
		flex-direction: column;
		gap: var(--space-2);
		padding: var(--space-2);
		border-top: var(--border-width) solid var(--border);
		background: var(--surface-1);
	}

	textarea {
		padding: var(--space-2) var(--space-3);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--color-bg);
		color: var(--color-fg);
		font: inherit;
		resize: none;
	}

	textarea:focus-visible {
		outline: var(--border-width) solid var(--status-working);
	}

	.actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-2);
	}
</style>
