<script lang="ts">
	import { agentNames } from '#lib/config.svelte.ts';
	import Chips from '#lib/chat/Chips.svelte';
	import ContextBar from '#lib/chat/ContextBar.svelte';
	import FileMenu from '#lib/chat/FileMenu.svelte';
	import SlashMenu from '#lib/chat/SlashMenu.svelte';
	import type { Builtin, Capabilities, Chat, Choice, Device, Image, Request, Setting } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';
	import Composer from '#lib/shell/Composer.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import type { Component } from 'svelte';
	import ArrowBendUpRight from '~icons/ph/arrow-bend-up-right';
	import At from '~icons/ph/at';
	import BookmarkSimple from '~icons/ph/bookmark-simple';
	import DotsThree from '~icons/ph/dots-three';
	import ListPlus from '~icons/ph/list-plus';
	import PaperPlaneRight from '~icons/ph/paper-plane-right';
	import Paperclip from '~icons/ph/paperclip';
	import Stop from '~icons/ph/stop-fill';
	import Terminal from '~icons/ph/terminal';
	import X from '~icons/ph/x';

	let { device, chat, live, request, capabilities, choices, branches, history, shelling, focused = false, onrun }: {
		device: Device;
		chat: Chat;
		live: boolean;
		branches: { label: string; icon: Component; disabled: boolean; onselect(): void }[];
		history: string[];
		shelling: boolean;
		focused?: boolean;
		request?: Request;
		capabilities: Capabilities;
		choices: Record<Setting, Choice[]>;
		onrun(command: Builtin, value?: string | number): void;
	} = $props();

	let composer: Composer;
	let slash = $state<SlashMenu>();
	let files = $state<FileMenu>();
	let picker: HTMLInputElement;
	let images = $state<(Image & { name: string })[]>([]);
	let error = $state('');
	let reading = $state(false);

	function choose() {
		const files = [...(picker.files ?? [])];
		picker.value = '';
		void attach(files);
	}

	function paste(event: ClipboardEvent) {
		const files = [...(event.clipboardData?.items ?? [])]
			.filter((item) => item.kind === 'file' && item.type.startsWith('image/'))
			.map((item) => item.getAsFile())
			.filter((file) => file !== null);
		if (!files.length) return;
		event.preventDefault();
		void attach(files);
	}

	export async function attach(files: File[]) {
		if (!live || reading || request || !files.length) return;
		error = '';
		if (files.length + images.length > 20) { error = 'Attach up to 20 images.'; return; }
		if (files.some((file) => !['image/png', 'image/jpeg', 'image/gif', 'image/webp'].includes(file.type))) { error = 'Choose PNG, JPEG, GIF or WebP images.'; return; }
		if (files.some((file) => file.size > 5 * 1024 * 1024)) { error = 'Each image must be 5 MB or smaller.'; return; }
		if (files.reduce((size, file) => size + file.size, 0) + images.reduce((size, image) => size + image.data.length * 3 / 4, 0) > 20 * 1024 * 1024) { error = 'Keep attachments under 20 MB per message.'; return; }
		reading = true;
		try {
			const added = await Promise.all(files.map((file) => new Promise<Image & { name: string }>((resolve, reject) => {
				const reader = new FileReader();
				reader.onload = () => resolve({ name: file.name, mediaType: file.type, data: String(reader.result).split(',')[1] });
				reader.onerror = () => reject(reader.error);
				reader.readAsDataURL(file);
			})));
			images = [...images, ...added];
		} catch { error = 'Could not read the image. Try again.'; }
		finally { reading = false; }
	}

	const working = $derived(chat.status !== 'done' && !request);
	const stoppable = $derived(working || shelling);
	const agent = $derived(chat.agent === 'claude-code' ? 'Claude' : agentNames[chat.agent]);

	function send(text: string, queue: boolean) {
		if (!live || reading || (!text.trim() && !images.length)) return false;
		if (images.length > 20) { error = 'Attach up to 20 images.'; return false; }
		if (images.reduce((size, image) => size + image.data.length * 3 / 4, 0) > 20 * 1024 * 1024) { error = 'Keep attachments under 20 MB per message.'; return false; }
		if (text.trimStart().startsWith('!')) {
			const command = text.trim().slice(1).trim();
			if (!command) return false;
			if (images.length) { error = 'Remove images before running a command.'; return false; }
			device.send({ type: 'chat.shell', chatId: chat.id, command });
			error = '';
			return true;
		}
		if (request) {
			if (images.length) { error = 'Remove images before answering a request.'; return false; }
			device.send({ type: 'chat.answer', chatId: chat.id, requestId: request.id, answer: { allow: false, instead: text } });
			return true;
		}
		const [, command, value] = text.trim().match(/^\/(\w+)(?: +(\S+))?$/) ?? [];
		if (!images.length && (command === 'compact' || command === 'clear' || command === 'rewind')) {
			if (chat.status !== 'done' || value) return false;
			onrun(command);
			return true;
		}
		if (!images.length && (command === 'model' || command === 'effort' || command === 'mode')) {
			if (!choices[command].some((choice) => choice.value === value)) return false;
			onrun(command, value);
			return true;
		}
		device.send({ type: 'chat.send', chatId: chat.id, text, queue, images: images.map(({ mediaType, data }) => ({ mediaType, data })) });
		images = [];
		error = '';
		return true;
	}

	function interrupt() {
		composer.prepend(chat.queued);
		device.send({ type: 'chat.interrupt', chatId: chat.id });
	}

	export function restore(text: string, attachments: Image[]) {
		composer.prepend([text]);
		images = [...attachments.map((image, index) => ({ ...image, name: `Attached image ${index + 1}` })), ...images];
		composer.focus();
	}

	function keydown(event: KeyboardEvent) {
		slash?.key(event);
		if (!event.defaultPrevented) files?.key(event);
		if (event.defaultPrevented || !live) return;
		if (event.key === 'Escape' && stoppable) interrupt();
		else if (event.key === 'Tab' && event.shiftKey && choices.mode.length) {
			const at = choices.mode.findIndex((choice) => choice.value === chat.settings.mode);
			onrun('mode', choices.mode[(at + 1) % choices.mode.length].value);
		} else return;
		event.preventDefault();
	}

	function keepFocus(event: MouseEvent) {
		event.preventDefault();
	}
</script>

<input bind:this={picker} type="file" accept="image/png,image/jpeg,image/gif,image/webp" multiple aria-label="Attach images" hidden onchange={choose} />
{#if images.length}
	<div class="attachments" class:phone={form() === 'phone'} aria-label="Attachments">
		{#each images as image, index}
			<div class="thumbnail"><img src="data:{image.mediaType};base64,{image.data}" alt={image.name} /><button aria-label="Remove {image.name}" onclick={() => (images = images.filter((_, at) => at !== index))}><X /></button></div>
		{/each}
	</div>
{/if}
{#if error}<div role="alert">{error}</div>{/if}
<Composer bind:this={composer} id={chat.id} draft={chat.draft} {focused} placeholder={request ? `Or tell ${agent} what to do instead…` : form() === 'phone' ? undefined : '/ commands · @ files · ! shell · ; clips'} {history} code={(text) => text.trimStart().startsWith('!')} onsend={(text) => send(text, false)} onkeydown={keydown} onpaste={paste}>
	{#snippet menu(text, replace, caret)}
		<SlashMenu bind:this={slash} {text} agent={chat.agent} settings={chat.settings} {capabilities} {choices} working={chat.status !== 'done'} {replace} {onrun} />
		{#if !slash?.isOpen()}
			<FileMenu bind:this={files} {text} {caret} projectId={chat.projectId} {replace} />
		{/if}
	{/snippet}
	{#snippet actions(text: string, clear: () => void)}
		{#if form() === 'phone'}
			<Button aria-label="Commands" tip="Commands" aria-expanded={slash?.isOpen() ?? false} onmousedown={keepFocus} onclick={() => slash?.toggle()}><Terminal /></Button>
			<Button aria-label="Reference file" tip="Reference file" aria-expanded={files?.isOpen() ?? false} onmousedown={keepFocus} onclick={() => (files?.isOpen() ? files.dismiss() : composer.insert('@'))}><At /></Button>
		{:else}
			<Chips agent={chat.agent} settings={chat.settings} {choices} docked {onrun}>
				{#if chat.context}<ContextBar context={chat.context} tokens={chat.tokens} />{/if}
			</Chips>
		{/if}
		<Button aria-label="Attach image" tip="Attach image" disabled={!live || reading || !!request} onclick={() => picker.click()}><Paperclip /></Button>
		<Menu items={[{ label: 'Save as Clip', icon: BookmarkSimple, disabled: !text.trim(), onselect: () => composer.saveAsClip() }, ...branches]}>
			{#snippet trigger(props)}
				<Button {...props} aria-label="More" tip="More"><DotsThree /></Button>
			{/snippet}
		</Menu>
		<div class="turn">
			{#if stoppable}
				<Button aria-label="Interrupt" tip="Interrupt" disabled={!live} onmousedown={keepFocus} onclick={interrupt}>
					<span class="stop"><Stop /></span>
				</Button>
				{#if form() === 'phone'}<span class="gap"></span>{/if}
			{/if}
			{#if text.trimStart().startsWith('!')}
				<Button variant="primary" disabled={!text.trim().slice(1).trim() || !live || reading} onmousedown={keepFocus} onclick={() => send(text, false) && clear()}>
					<Terminal /> Run
				</Button>
			{:else if working}
				<Button disabled={(!text.trim() && !images.length) || !live || reading} onmousedown={keepFocus} onclick={() => send(text, true) && clear()}>
					<ListPlus /> Queue
				</Button>
				<Button
					variant="primary"
					disabled={(!text.trim() && !images.length) || !live || reading}
					onmousedown={keepFocus}
					onclick={() => send(text, false) && clear()}
				>
					<ArrowBendUpRight /> Steer
				</Button>
			{:else}
				<Button
					variant="primary"
					aria-label="Send"
					tip="Send"
					style="min-width: calc(2 * (1.2em + 2 * var(--space-3) + 2 * var(--border-width)))"
					disabled={(!text.trim() && !images.length) || !live || reading}
					onmousedown={keepFocus}
					onclick={() => send(text, false) && clear()}
				>
					<PaperPlaneRight />
				</Button>
			{/if}
		</div>
	{/snippet}
</Composer>

<style>
	.attachments { display: flex; gap: var(--space-2); overflow-x: auto; }
	.thumbnail { position: relative; flex: none; }
	.thumbnail img { width: 64px; height: 64px; object-fit: cover; border-radius: var(--radius-md); }
	.thumbnail button { position: absolute; top: 0; right: 0; min-width: var(--target-min); min-height: var(--target-min); border: 0; border-radius: var(--radius-md); background: var(--surface-2); color: var(--color-fg); cursor: pointer; }
	.phone .thumbnail { display: flex; align-items: center; gap: var(--space-1); }
	.phone .thumbnail button { position: static; display: inline-grid; place-items: center; }
	[role='alert'] { color: var(--ansi-1); }
	.stop {
		display: inline-flex;
		color: var(--ansi-1);
	}

	.gap {
		flex: 1;
	}

	.turn {
		display: contents;
	}

	.turn > :global(button:has(.stop)) {
		border-color: transparent;
		background: none;
	}

	.turn > :global(button:has(.stop):hover:not(:disabled)) {
		background: var(--hover);
	}
</style>
