<script lang="ts">
	import { agentNames } from '#lib/config.svelte.ts';
	import Handoff from '#lib/chat/Handoff.svelte';
	import SlashMenu from '#lib/chat/SlashMenu.svelte';
	import type { Builtin, Capabilities, Chat, Choice, Device, Image, Request, Setting } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';
	import Composer from '#lib/shell/Composer.svelte';
	import Button from '#lib/ui/Button.svelte';
	import ArrowBendUpRight from '~icons/ph/arrow-bend-up-right';
	import ListPlus from '~icons/ph/list-plus';
	import PaperPlaneRight from '~icons/ph/paper-plane-right';
	import Paperclip from '~icons/ph/paperclip';
	import Stop from '~icons/ph/stop-fill';
	import Terminal from '~icons/ph/terminal';
	import X from '~icons/ph/x';

	let { device, chat, live, request, capabilities, choices, handoffAvailable, focused = false, onrun }: {
		device: Device;
		chat: Chat;
		live: boolean;
		handoffAvailable: boolean;
		focused?: boolean;
		request?: Request;
		capabilities: Capabilities;
		choices: Record<Setting, Choice[]>;
		onrun(command: Builtin, value?: string | number): void;
	} = $props();

	let composer: Composer;
	let slash = $state<SlashMenu>();
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

	async function attach(files: File[]) {
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
	const agent = $derived(chat.agent === 'claude-code' ? 'Claude' : agentNames[chat.agent]);

	function send(text: string, queue: boolean) {
		if (!live || reading || (!text.trim() && !images.length)) return false;
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

	function keepFocus(event: MouseEvent) {
		event.preventDefault();
	}
</script>

<input bind:this={picker} type="file" accept="image/png,image/jpeg,image/gif,image/webp" multiple aria-label="Attach images" hidden onchange={choose} />
{#if images.length}
	<div class="attachments" aria-label="Attachments">
		{#each images as image, index}
			<div class="thumbnail"><img src="data:{image.mediaType};base64,{image.data}" alt={image.name} /><button aria-label="Remove {image.name}" onclick={() => (images = images.filter((_, at) => at !== index))}><X /></button></div>
		{/each}
	</div>
{/if}
{#if error}<div role="alert">{error}</div>{/if}
<Composer bind:this={composer} id={chat.id} draft={chat.draft} {focused} placeholder={request ? `Or tell ${agent} what to do instead…` : `Message ${chat.rig}`} onsend={(text) => send(text, false)} onkeydown={(event) => slash?.key(event)} onpaste={paste}>
	{#snippet menu(text, replace)}
		<SlashMenu bind:this={slash} {text} agent={chat.agent} settings={chat.settings} {capabilities} {choices} working={chat.status !== 'done'} {replace} {onrun} />
	{/snippet}
	{#snippet actions(text: string, clear: () => void)}
		{#if form() === 'phone'}
			<Button aria-label="Commands" title="Commands" aria-expanded={slash?.isOpen() ?? false} onmousedown={keepFocus} onclick={() => slash?.toggle()}><Terminal /></Button>
		{/if}
		<Button aria-label="Attach image" title="Attach image" disabled={!live || reading || !!request} onclick={() => picker.click()}><Paperclip /></Button>
		<Handoff {device} {chat} available={handoffAvailable} />
		{#if working}
			<Button aria-label="Interrupt" title="Interrupt" disabled={!live} onmousedown={keepFocus} onclick={interrupt}>
				<span class="stop"><Stop /></span>
			</Button>
			<span class="gap"></span>
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
				title="Send"
				style="min-width: calc(2 * (1.2em + 2 * var(--space-3) + 2 * var(--border-width)))"
				disabled={(!text.trim() && !images.length) || !live || reading}
				onmousedown={keepFocus}
				onclick={() => send(text, false) && clear()}
			>
				<PaperPlaneRight />
			</Button>
		{/if}
	{/snippet}
</Composer>

<style>
	.attachments { display: flex; gap: var(--space-2); overflow-x: auto; }
	.thumbnail { position: relative; flex: none; }
	.thumbnail img { width: 64px; height: 64px; object-fit: cover; border-radius: var(--radius-md); }
	.thumbnail button { position: absolute; top: 0; right: 0; min-width: var(--target-min); min-height: var(--target-min); border: 0; border-radius: var(--radius-md); background: var(--surface-2); color: var(--color-fg); cursor: pointer; }
	[role='alert'] { color: var(--ansi-1); }
	.stop {
		display: inline-flex;
		color: var(--ansi-1);
	}

	.gap {
		flex: 1;
	}
</style>
