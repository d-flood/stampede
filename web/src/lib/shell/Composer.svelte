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
		/** Returns how many leading characters to mark as a recognized command. */
		mark?: (text: string) => number;
		menu?: Snippet<[text: string, replace: (text: string, caret?: number) => void, caret: number]>;
		onkeydown?: (event: KeyboardEvent) => void;
		onpaste?: (event: ClipboardEvent) => void;
		onsend?(text: string, enter: boolean): boolean;
		/** Replaces Insert and Send, and boxes the Composer in a Chat's column. */
		actions?: Snippet<[text: string, clear: () => void]>;
		border?: Snippet<[text: string]>;
		footer?: Snippet;
	};

	let { id, disabled = false, focused = false, placeholder, draft, code = false, history = [], mark, onsend, actions, border, footer, menu, onkeydown, onpaste }: Props = $props();

	const key = $derived(`composer:${id}`);
	let text = $derived(localStorage.getItem(key) ?? draft ?? '');
	let box: HTMLTextAreaElement;
	let caret = $state(0);
	const coded = $derived(typeof code === 'function' ? code(text) : code);
	const marked = $derived(coded ? 0 : (mark?.(text) ?? 0));
	let mirror = $state<HTMLDivElement>();
	let clipMenu = $state<ClipMenu>();
	let clipping = $state(false);
	const phone = $derived(form() === 'phone');

	function keep() {
		if (text || draft) localStorage.setItem(key, text);
		else localStorage.removeItem(key);
	}

	export function clear() {
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

	export function saveAsClip() {
		clipping = true;
	}

	export function prepend(lines: string[]) {
		text = [...lines, text].filter(Boolean).join('\n\n');
		keep();
	}
</script>

{#snippet editor()}
	<textarea
		{@attach (element) => { if (focused && !phone) element.focus(); }}
		bind:this={box}
		bind:value={text}
		oninput={(event) => { keep(); track(event); }}
		onkeydown={keydown}
		onkeyup={track}
		onpointerup={track}
		onfocus={track}
		onscroll={() => mirror && (mirror.scrollTop = box.scrollTop)}
		{onpaste}
		aria-label="Composer"
		class:code={coded}
		{placeholder}
		rows={actions ? 1 : 3}
		autocorrect={coded ? 'off' : 'on'}
		autocapitalize={coded ? 'off' : 'sentences'}
		spellcheck={!coded}
	></textarea>
	{#if marked}
		<!-- Trails a space so a final newline still adds a line, as it does in the textarea. -->
		<div bind:this={mirror} class="mirror" aria-hidden="true" {@attach (node) => { node.scrollTop = box.scrollTop; }}><mark>{text.slice(0, marked)}</mark>{`${text.slice(marked)} `}</div>
	{/if}
{/snippet}

<div class="composer" class:boxed={actions} class:phone>
	{@render menu?.(text, replace, caret)}
	<ClipMenu bind:this={clipMenu} {text} {caret} {replace} />
	{#if actions && phone}
		<div class="card">
			<div class="field">{@render editor()}</div>
			<div class="actions">{@render actions(text, clear)}</div>
			{#if border}<div class="outline">{@render border(text)}</div>{/if}
		</div>
		{#if footer}<div class="footer">{@render footer()}</div>{/if}
	{:else}
		<div class="field">
			{@render editor()}
			{@render border?.(text)}
		</div>
		<div class="actions">
			{#if !actions && text.trim()}
				<Button aria-label="Save as Clip" tip="Save as Clip" onmousedown={(event) => event.preventDefault()} onclick={() => (clipping = true)}><BookmarkSimple /></Button>
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
	{/if}
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

	.field {
		position: relative;
		display: grid;
	}

	textarea, .mirror {
		padding: var(--space-2) var(--space-3);
	}

	textarea {
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--color-bg);
		color: var(--color-fg);
		font: inherit;
		resize: none;
	}

	textarea:focus-visible {
		outline: var(--border-width) solid var(--border-strong);
	}

	.mirror {
		position: absolute;
		inset: 0;
		overflow: hidden;
		border: var(--border-width) solid transparent;
		color: transparent;
		white-space: pre-wrap;
		overflow-wrap: break-word;
		pointer-events: none;
	}

	mark {
		margin: 0 calc(-1 * var(--space-1));
		padding: 0 var(--space-1);
		border-radius: var(--radius-sm);
		background: var(--selected-bg);
		color: transparent;
	}

	textarea.code {
		font-family: var(--font-mono);
		font-size: var(--font-size-code);
	}

	.actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-2);
	}

	.boxed {
		padding: 0;
		border: none;
		background: none;
	}

	.boxed textarea, .boxed .mirror {
		padding: var(--space-3) var(--space-4);
		align-content: safe center;
	}

	.boxed textarea {
		min-height: 2lh;
		max-height: 40vh;
		border: var(--border-width) solid var(--border-strong);
		border-radius: var(--radius-xl);
		background: var(--surface-1);
		box-shadow: var(--shadow-lift);
		outline: none;
		field-sizing: content;
	}

	.boxed .actions {
		align-items: center;
		flex-wrap: wrap;
	}

	.boxed.phone {
		gap: var(--space-2);
	}

	.card {
		position: relative;
		display: flex;
		flex-direction: column;
		border: var(--border-width) solid var(--border-strong);
		border-radius: var(--radius-xl);
		background: var(--surface-1);
	}

	.outline {
		position: absolute;
		inset: calc(-1 * var(--border-width));
		pointer-events: none;
	}

	.footer {
		display: flex;
		align-items: center;
		min-width: 0;
	}

	.boxed.phone .actions {
		justify-content: space-between;
		flex-wrap: nowrap;
		gap: var(--space-2);
		padding: 0 var(--space-2) var(--space-2);
	}

	.boxed.phone textarea, .boxed.phone .mirror {
		border: 0;
		border-radius: var(--radius-xl) var(--radius-xl) 0 0;
		background: none;
		box-shadow: none;
		font-size: var(--font-size-lg);
	}

	.boxed.phone textarea {
		min-height: 0;
		max-height: 36vh;
	}
</style>
