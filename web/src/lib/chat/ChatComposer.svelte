<script lang="ts">
	import { agentNames } from '#lib/config.svelte.ts';
	import { cold, warmth } from '#lib/chat/cache.ts';
	import Chips from '#lib/chat/Chips.svelte';
	import { builtins, recognize } from '#lib/chat/commands.ts';
	import ContextBar from '#lib/chat/ContextBar.svelte';
	import { count } from '#lib/chat/count.ts';
	import FileMenu from '#lib/chat/FileMenu.svelte';
	import { pastedImages } from '#lib/chat/images.ts';
	import { redirect } from '#lib/chat/questions.ts';
	import RigPicker from '#lib/chat/RigPicker.svelte';
	import { ago, logos } from '#lib/chat/sessions.ts';
	import SlashMenu from '#lib/chat/SlashMenu.svelte';
	import TurnRing from '#lib/chat/TurnRing.svelte';
	import type { Builtin, Capabilities, Chat, Choice, Device, Image, Request, Setting, Settings } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';
	import { hostOf } from '#lib/host.ts';
	import Composer from '#lib/shell/Composer.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import type { Component } from 'svelte';
	import { prefersReducedMotion } from 'svelte/motion';
	import { fade, fly } from 'svelte/transition';
	import ArrowBendUpRight from '~icons/ph/arrow-bend-up-right';
	import At from '~icons/ph/at';
	import BookmarkSimple from '~icons/ph/bookmark-simple';
	import DotsThree from '~icons/ph/dots-three';
	import ListPlus from '~icons/ph/list-plus';
	import PaperPlaneRight from '~icons/ph/paper-plane-right';
	import Paperclip from '~icons/ph/paperclip';
	import Play from '~icons/ph/play';
	import Plus from '~icons/ph/plus';
	import Stop from '~icons/ph/stop-fill';
	import Terminal from '~icons/ph/terminal';
	import X from '~icons/ph/x';

	let { device, chat, live, request, capabilities, settings, ready, choices, rigs, branches, history, shelling, shell, focused = false, onreach, onrun }: {
		device: Device;
		chat: Chat;
		live: boolean;
		shell?: string;
		branches: { label: string; icon: Component; disabled: boolean; onselect(): void }[];
		history: string[];
		shelling: boolean;
		focused?: boolean;
		request?: Request;
		capabilities: Capabilities;
		settings: Settings;
		ready: boolean;
		choices: Record<Setting, Choice[]>;
		/** The Rigs a Dormant Chat can resume with. */
		rigs?: string[];
		onreach?(): void;
		onrun(command: Builtin, value?: string | number): void;
	} = $props();

	let composer: Composer;
	let slash = $state<SlashMenu>();
	let files = $state<FileMenu>();
	let picker: HTMLInputElement;
	let images = $state<(Image & { name: string })[]>([]);
	let error = $state('');
	let reading = $state(false);
	let launching = $state(false);
	let confirming = $state<{ text: string; queue: boolean; probably: boolean }>();
	let focusComposer = false;
	const phone = $derived(form() === 'phone');
	const imageable = $derived(!request || (request.kind === 'question' && !!request.questions[0]?.type));

	function choose() {
		const files = [...(picker.files ?? [])];
		picker.value = '';
		void attach(files);
	}

	function paste(event: ClipboardEvent) {
		const files = pastedImages(event);
		if (!files.length) return;
		event.preventDefault();
		void attach(files);
	}

	export async function attach(files: File[]) {
		if (!live || reading || !imageable || !files.length) return;
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
	const command = (text: string) => recognize(text, capabilities, choices);

	function send(text: string, queue: boolean, confirmed = false) {
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
			if (images.length && !imageable) { error = 'Remove images before answering a request.'; return false; }
			device.send({ type: 'chat.answer', chatId: chat.id, requestId: request.id, answer: { ...redirect(request, text), ...(images.length && { images: take() }) } });
			error = '';
			return true;
		}
		const typed = text.trim().match(/^\/(\w+)/)?.[1];
		if (typed && ['clear', 'reset', 'new'].includes(typed) && !capabilities.commands.some(({ name }) => name === typed)) {
			error = `/${typed} isn't available here. Open a new Chat instead.`;
			return false;
		}
		const [, command, value] = text.trim().match(/^\/(\w+)(?: +(\S+))?$/) ?? [];
		if (!images.length && (command === 'compact' || command === 'rewind') && builtins(capabilities).includes(command)) {
			if (chat.status !== 'done' || value) return false;
			onrun(command);
			return true;
		}
		if (!images.length && (command === 'model' || command === 'effort' || command === 'speed' || command === 'ultracode' || command === 'mode')) {
			const choice = choices[command].find((choice) => choice.value === value || choice.label === value);
			if (!choice) return false;
			onrun(command, choice.value);
			return true;
		}
		const lapse = chat.context?.cache && warmth(chat.context.cache);
		if (!confirmed && !working && cold(lapse) && chat.context!.used >= 100_000) {
			confirming = { text, queue, probably: lapse === 'probably-cold' };
			return false;
		}
		launching = !working && !prefersReducedMotion.current;
		device.send({ type: 'chat.send', chatId: chat.id, text, queue, images: images.map(({ mediaType, data }) => ({ mediaType, data })) });
		images = [];
		error = '';
		return true;
	}

	function confirm() {
		const { text, queue } = confirming!;
		confirming = undefined;
		if (send(text, queue, true)) composer.clear();
	}

	function interrupt() {
		composer.prepend(chat.queued);
		device.send({ type: 'chat.interrupt', chatId: chat.id });
	}

	export function take() {
		const taken = images.map(({ mediaType, data }) => ({ mediaType, data }));
		images = [];
		return taken;
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
		if (event.key !== 'Escape' || !stoppable) return;
		interrupt();
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
{#snippet meta()}
	{#if rigs?.length}
		<RigPicker rigs={rigs.map((label) => ({ label, icon: logos[chat.agent] }))} current={rigs.indexOf(chat.rig)} onpick={(index) => device.send({ type: 'chat.rig', chatId: chat.id, rig: rigs[index] })} />
	{/if}
	<!-- svelte-ignore a11y_no_static_element_interactions (Hover and focus only start loading the model list.) -->
	<span class="tuning" onpointerover={onreach} onfocusin={onreach}>
		<Chips agent={chat.agent} {settings} {ready} {choices} mark={!rigs?.length} {onrun}>
			{#if chat.context}<ContextBar context={chat.context} tokens={chat.tokens} />{/if}
		</Chips>
	</span>
{/snippet}

<Composer bind:this={composer} id={chat.id} draft={chat.draft} {focused} placeholder={request ? `Or tell ${agent} what to do instead…` : chat.dormant ? 'Message to resume this Session' : `Message ${agent}`} {history} code={(text) => text.trimStart().startsWith('!')} mark={(text) => command(text)?.token.length ?? 0} footer={phone ? meta : undefined} onsend={(text) => send(text, false)} onkeydown={keydown} onpaste={paste}>
	{#snippet border(text: string)}
		{@const known = chat.status === 'done' && !shell && !slash?.isOpen() && !files?.isOpen() ? command(text) : undefined}
		<TurnRing status={chat.status} activity={chat.activity} {shell} />
		{#if known}
			<span class="frame" aria-hidden="true" transition:fade={{ duration: 150 }}></span>
			<span class="legend" in:fly={{ y: prefersReducedMotion.current ? 0 : 4, duration: 200 }} out:fade={{ duration: 150 }}>
				<span class="name">{known.token}</span>
				{#if known.detail}<span class="detail">· {known.detail}</span>{/if}
			</span>
		{/if}
	{/snippet}
	{#snippet menu(text, replace, caret)}
		<SlashMenu bind:this={slash} {text} agent={chat.agent} {settings} {capabilities} {choices} working={chat.status !== 'done'} cloud={!!hostOf(chat.projectId)} {replace} {onrun} />
		{#if !slash?.isOpen()}
			<FileMenu bind:this={files} {text} {caret} projectId={chat.projectId} {replace} />
		{/if}
	{/snippet}
	{#snippet actions(text: string, clear: () => void)}
		{@const hasDraft = !!text.trim() || !!images.length}
		{@const interruptOnly = stoppable && !hasDraft && !launching}
		{#if phone}
			<div class="group">
				<Menu
					side="top"
					align="start"
					items={[
						{
							heading: 'Insert',
							items: [
								{ label: 'Commands', icon: Terminal, hint: '/', onselect: () => { focusComposer = true; slash?.toggle(); } },
								{ label: 'Reference file', icon: At, hint: '@', onselect: () => { focusComposer = true; if (files?.isOpen()) files.dismiss(); else composer.insert('@'); } }
							]
						},
						{
							heading: 'Chat',
							items: [{ label: 'Save as Clip', icon: BookmarkSimple, disabled: !text.trim(), onselect: () => composer.saveAsClip() }, ...branches]
						}
					]}
					oncloseautofocus={(event) => {
						if (!focusComposer) return;
						focusComposer = false;
						event.preventDefault();
						composer.focus();
					}}
				>
					{#snippet trigger(props)}
						<Button {...props} aria-label="More"><Plus /></Button>
					{/snippet}
				</Menu>
				<Button variant="ghost" aria-label="Attach image" disabled={!live || reading || !imageable} onclick={() => picker.click()}><Paperclip /></Button>
			</div>
		{:else}
			{@render meta()}
			<Button variant="ghost" aria-label="Attach image" tip="Attach image" disabled={!live || reading || !imageable} onclick={() => picker.click()}><Paperclip /></Button>
			<Menu side="top" items={[{ label: 'Save as Clip', icon: BookmarkSimple, disabled: !text.trim(), onselect: () => composer.saveAsClip() }, ...branches]}>
				{#snippet trigger(props)}
					<Button {...props} variant="ghost" aria-label="More" tip="More"><DotsThree /></Button>
				{/snippet}
			</Menu>
		{/if}
		<div class={phone ? 'phone-turn' : 'turn'}>
			{#if interruptOnly}
				<Button variant="danger" disabled={!live} onmousedown={keepFocus} onclick={interrupt}>
					<Stop /> Interrupt
				</Button>
			{:else}
				{#if stoppable}
					<Button variant="danger" aria-label="Interrupt" tip={phone ? undefined : 'Interrupt'} disabled={!live} onmousedown={keepFocus} onclick={interrupt}>
						<Stop />
					</Button>
					{#if phone}<span class="divider" aria-hidden="true"></span>{/if}
				{/if}
				{#if text.trimStart().startsWith('!')}
					<Button variant="primary" disabled={!text.trim().slice(1).trim() || !live || reading} onmousedown={keepFocus} onclick={() => send(text, false) && clear()}>
						<Terminal /> Run
					</Button>
				{:else if !request && !images.length && command(text)?.builtin}
					<Button variant="primary" disabled={!live || reading} onmousedown={keepFocus} onclick={() => send(text, false) && clear()}>
						<Play /> Run
					</Button>
				{:else if working && !launching}
					<Button aria-label={phone ? 'Queue' : undefined} disabled={!hasDraft || !live || reading} onmousedown={keepFocus} onclick={() => send(text, true) && clear()}>
						<ListPlus />{#if !phone} Queue{/if}
					</Button>
					<Button
						variant="primary"
						aria-label={phone ? 'Steer' : undefined}
						disabled={!hasDraft || !live || reading}
						onmousedown={keepFocus}
						onclick={() => send(text, false) && clear()}
					>
						<ArrowBendUpRight />{#if !phone} Steer{/if}
					</Button>
				{:else}
					<Button
						variant="primary"
						round
						aria-label="Send"
						tip={phone ? undefined : 'Send'}
						disabled={!hasDraft || !live || reading}
						onmousedown={keepFocus}
						onclick={() => send(text, false) && clear()}
					>
						<span class="plane" class:launching onanimationend={() => (launching = false)}><PaperPlaneRight /></span>
					</Button>
				{/if}
			{/if}
		</div>
	{/snippet}
</Composer>

<Dialog
	bind:open={() => !!confirming, (open) => !open && (confirming = undefined)}
	title={confirming?.probably ? 'Cache probably cold' : 'Cache cold'}
	description={chat.context?.cache
		? `The prompt cache ${confirming?.probably ? 'probably expired' : 'expired'} ${ago(chat.context.cache.expires)}, so this message ${confirming?.probably ? 'probably re-reads' : 're-reads'} all ${count(chat.context.used)} tokens of context uncached.`
		: ''}
	oncloseautofocus={(event) => {
		event.preventDefault();
		composer.focus();
	}}
>
	{#snippet actions()}
		<Button onclick={() => (confirming = undefined)}>Cancel</Button>
		<Button variant="primary" onclick={confirm}>Send anyway</Button>
	{/snippet}
</Dialog>

<style>
	.tuning {
		display: flex;
		flex: 1 1 auto;
		min-width: 0;
	}

	.attachments { display: flex; gap: var(--space-2); overflow-x: auto; }
	.thumbnail { position: relative; flex: none; }
	.thumbnail img { width: 64px; height: 64px; object-fit: cover; border-radius: var(--radius-md); }
	.thumbnail button { position: absolute; top: 0; right: 0; min-width: var(--target-min); min-height: var(--target-min); border: 0; border-radius: var(--radius-md); background: var(--surface-2); color: var(--color-fg); cursor: pointer; }
	.phone .thumbnail { display: flex; align-items: center; gap: var(--space-1); }
	.phone .thumbnail button { position: static; display: inline-grid; place-items: center; }
	[role='alert'] { color: var(--ansi-1); }

	.group,
	.phone-turn {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}

	.group :global(button),
	.phone-turn :global(button) {
		min-width: var(--target-min);
	}

	.divider {
		flex: none;
		width: var(--border-width);
		height: calc(var(--target-min) - 2 * var(--space-4));
		background: var(--border);
	}

	.plane {
		display: inline-flex;
	}

	.launching {
		animation: launch 450ms var(--ease-out);
	}

	@keyframes launch {
		45% { opacity: 0; translate: 1.2em -0.9em; }
		46% { opacity: 0; translate: -1.2em 0.9em; }
	}

	.turn {
		display: contents;
	}

	.frame {
		position: absolute;
		inset: 0;
		border: var(--border-width) solid var(--selected-border);
		border-radius: var(--radius-xl);
		box-shadow: 0 0 0 var(--border-width) var(--selected-border);
		pointer-events: none;
	}

	.legend {
		position: absolute;
		top: calc(var(--border-width) / 2);
		left: var(--radius-xl);
		z-index: 1;
		display: flex;
		gap: var(--space-1);
		max-width: calc(100% - 2 * var(--radius-xl));
		padding: 0 var(--space-2);
		translate: 0 -50%;
		background: linear-gradient(var(--color-bg) 50%, var(--card) 0);
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		line-height: 1.4;
		white-space: nowrap;
		pointer-events: none;
	}

	.name {
		flex: none;
		color: var(--accent);
		font-family: var(--font-mono);
	}

	.detail {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
	}
</style>
