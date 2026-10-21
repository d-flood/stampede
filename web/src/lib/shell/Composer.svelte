<script lang="ts">
	import { form } from '#lib/form.ts';
	import ClipEditor from '#lib/shell/ClipEditor.svelte';
	import ClipMenu from '#lib/shell/ClipMenu.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import { tick, type Snippet } from 'svelte';
	import BookmarkSimple from '~icons/ph/bookmark-simple';

	type Props = {
		id: string;
		disabled?: boolean;
		focused?: boolean;
		placeholder?: string;
		draft?: string;
		code?: boolean | ((text: string) => boolean);
		history?: string[];
		menu?: Snippet<[text: string, replace: (text: string, caret?: number) => void, caret: number]>;
		onkeydown?: (event: KeyboardEvent) => void;
		onpaste?: (event: ClipboardEvent) => void;
		onsend?(text: string, enter: boolean): boolean;
		/** Replaces Insert and Send, and boxes the Composer in a Chat's column. */
		actions?: Snippet<[text: string, clear: () => void]>;
	};

	let { id, disabled = false, focused = false, placeholder, draft, code = false, history = [], onsend, actions, menu, onkeydown, onpaste }: Props = $props();

	const key = $derived(`composer:${id}`);
	let text = $derived(localStorage.getItem(key) ?? draft ?? '');
	let box: HTMLTextAreaElement;
	let caret = $state(0);
	const coded = $derived(typeof code === 'function' ? code(text) : code);
	let clipMenu = $state<ClipMenu>();
	let clipping = $state(false);
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
		clipMenu?.key(event);
		if (!event.defaultPrevented) onkeydown?.(event);
		if (!event.defaultPrevented) recall(event);
		if (event.defaultPrevented || phone || event.key !== 'Enter' || event.shiftKey) return;
		event.preventDefault();
		if (!disabled && (text || actions)) submit(true);
	}

	function recall(event: KeyboardEvent) {
		const up = event.key === 'ArrowUp';
		if ((!up && event.key !== 'ArrowDown') || event.shiftKey || event.altKey || event.ctrlKey || event.metaKey) return;
		const at = text ? history.lastIndexOf(text) : history.length;
		const next = at + (up ? -1 : 1);
		const edge = !(up ? text.slice(0, box.selectionStart) : text.slice(box.selectionEnd)).includes('\n');
		if (at < 0 || next < 0 || next > history.length || !edge) return;
		event.preventDefault();
		replace(history[next] ?? '', up ? 0 : undefined);
	}

	function track(event: { currentTarget: HTMLTextAreaElement }) {
		caret = event.currentTarget.selectionStart;
	}

	async function replace(next: string, at = next.length) {
		text = next;
		keep();
		box.focus();
		await tick();
		box.setSelectionRange(at, at);
		caret = at;
	}

	export function insert(value: string) {
		const start = box.selectionStart;
		const spaced = start && !/\s/.test(text[start - 1]) ? ` ${value}` : value;
		replace(text.slice(0, start) + spaced + text.slice(box.selectionEnd), start + spaced.length);
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
	{@render menu?.(text, replace, caret)}
	<ClipMenu bind:this={clipMenu} {text} {caret} {replace} />
	<textarea
		{@attach (element) => { if (focused) element.focus(); }}
		bind:this={box}
		bind:value={text}
		oninput={(event) => { keep(); track(event); }}
		onkeydown={keydown}
		onkeyup={track}
		onpointerup={track}
		onfocus={track}
		{onpaste}
		aria-label="Composer"
		class:code={coded}
		{placeholder}
		rows={actions ? (phone ? 2 : 1) : 3}
		autocorrect={coded ? 'off' : 'on'}
		autocapitalize={coded ? 'off' : 'sentences'}
		spellcheck={!coded}
	></textarea>
	<div class="actions">
		{#if text.trim()}
			<Button aria-label="Save as Clip" title="Save as Clip" onmousedown={(event) => event.preventDefault()} onclick={() => (clipping = true)}><BookmarkSimple /></Button>
		{/if}
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
<Dialog bind:open={clipping} full={phone} title="Save as Clip">
	{#if clipping}<ClipEditor draft={text} ondone={() => (clipping = false)} />{/if}
</Dialog>

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

	textarea.code {
		font-family: var(--font-mono);
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
