<script lang="ts">
	import type { Chat, Device, Request } from '#lib/device.svelte.ts';
	import Composer from '#lib/shell/Composer.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Stop from '~icons/ph/stop-fill';

	let { device, chat, live, request }: { device: Device; chat: Chat; live: boolean; request?: Request } = $props();

	let composer: Composer;
	const working = $derived(chat.status !== 'done' && !request);
	const agent = $derived(chat.agent === 'codex' ? 'Codex' : 'Claude');

	function send(text: string, queue: boolean) {
		if (!live || !text.trim()) return false;
		if (request) device.send({ type: 'chat.answer', chatId: chat.id, requestId: request.id, answer: { allow: false, instead: text } });
		else device.send({ type: 'chat.send', chatId: chat.id, text, queue });
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

<Composer bind:this={composer} id={chat.id} placeholder={request ? `Or tell ${agent} what to do instead…` : `Message ${chat.rig}`}>
	{#snippet actions(text: string, clear: () => void)}
		{#if working}
			<Button aria-label="Interrupt" title="Interrupt" disabled={!live} onmousedown={keepFocus} onclick={interrupt}>
				<span class="stop"><Stop /></span>
			</Button>
			<span class="gap"></span>
			<Button disabled={!text || !live} onmousedown={keepFocus} onclick={() => send(text, true) && clear()}>
				Queue
			</Button>
			<Button
				variant="primary"
				disabled={!text || !live}
				onmousedown={keepFocus}
				onclick={() => send(text, false) && clear()}
			>
				Steer
			</Button>
		{:else}
			<Button
				variant="primary"
				disabled={!text || !live}
				onmousedown={keepFocus}
				onclick={() => send(text, false) && clear()}
			>
				Send
			</Button>
		{/if}
	{/snippet}
</Composer>

<style>
	.stop {
		display: inline-flex;
		color: var(--ansi-1);
	}

	.gap {
		flex: 1;
	}
</style>
