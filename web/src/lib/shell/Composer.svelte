<script lang="ts">
	import { form } from '#lib/form.ts';
	import Button from '#lib/ui/Button.svelte';
	import type { Snippet } from 'svelte';

	type Props = {
		id: string;
		disabled?: boolean;
		focused?: boolean;
		placeholder?: string;
		draft?: string;
		menu?: Snippet<[string, (text: string) => void]>;
		onkeydown?: (event: KeyboardEvent) => void;
		onpaste?: (event: ClipboardEvent) => void;
		onsend?(text: string, enter: boolean): boolean;
		/** Replaces Insert and Send, and boxes the Composer in a Chat's column. */
		actions?: Snippet<[text: string, clear: () => void]>;
	};

	let { id, disabled = false, focused = false, placeholder, draft, onsend, actions, menu, onkeydown, onpaste }: Props = $props();

	const key = $derived(`composer:${id}`);
	let text = $derived(localStorage.getItem(key) ?? draft ?? '');
	let box: HTMLTextAreaElement;
	const phone = $derived(form() === 'phone');

	function keep() {
		if (text || draft) localStorage.setItem(key, text);
		else localStorage.removeItem(key);
	}

	function clear() {
		text = '';
		keep();
		box.focus();
	}

	function submit(enter: boolean) {
		if (onsend?.(text, enter)) clear();
	}

	function keydown(event: KeyboardEvent) {
		if (event.isComposing) return;
		onkeydown?.(event);
		if (event.defaultPrevented || phone || event.key !== 'Enter' || event.shiftKey) return;
		event.preventDefault();
		if (!disabled && (text || actions)) submit(true);
	}

	function replace(next: string) {
		text = next;
		keep();
		box.focus();
	}

	export function focus() {
		box.focus();
	}

	export function prepend(lines: string[]) {
		text = [...lines, text].filter(Boolean).join('\n\n');
		keep();
	}
</script>

<div class="composer" class:boxed={actions} class:phone>
	{@render menu?.(text, replace)}
	<textarea
		{@attach (element) => { if (focused) element.focus(); }}
		bind:this={box}
		bind:value={text}
		oninput={keep}
		onkeydown={keydown}
		{onpaste}
		aria-label="Composer"
		{placeholder}
		rows={actions ? (phone ? 2 : 1) : 3}
		autocorrect="on"
		autocapitalize="sentences"
		spellcheck="true"
	></textarea>
	<div class="actions">
		{#if actions}
			{@render actions(text, clear)}
		{:else}
			<Button disabled={!text || disabled} onmousedown={(event) => event.preventDefault()} onclick={() => submit(false)}>
				Insert
			</Button>
			<Button
				variant="primary"
				disabled={!text || disabled}
				onmousedown={(event) => event.preventDefault()}
				onclick={() => submit(true)}
			>
				Send
			</Button>
		{/if}
	</div>
</div>

<style>
	.composer {
		position: relative;
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

	.boxed {
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
	}

	.boxed:focus-within {
		border-color: var(--status-working);
	}

	.boxed textarea {
		max-height: 40vh;
		padding: var(--space-1) var(--space-2);
		border: none;
		background: none;
		outline: none;
		field-sizing: content;
	}

	.boxed .actions {
		align-items: center;
		flex-wrap: wrap;
	}

	.boxed.phone {
		flex-direction: column;
		align-items: stretch;
		gap: var(--space-3);
		padding: 0;
		border: none;
		background: none;
	}

	.boxed.phone textarea {
		min-height: var(--target-min);
		padding: var(--space-3) var(--space-4);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--surface-1);
		font-size: var(--font-size-lg);
	}

	.boxed.phone textarea:focus-visible {
		border-color: var(--status-working);
	}
</style>
