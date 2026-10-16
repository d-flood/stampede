<script lang="ts">
	import SlashMenu from '#lib/chat/SlashMenu.svelte';
	import type { Builtin, Capabilities, Chat, Choice, Device, Request, Setting } from '#lib/device.svelte.ts';
	import Composer from '#lib/shell/Composer.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Stop from '~icons/ph/stop-fill';

	let { device, chat, live, request, capabilities, choices, onrun }: {
		device: Device;
		chat: Chat;
		live: boolean;
		request?: Request;
		capabilities: Capabilities;
		choices: Record<Setting, Choice[]>;
		onrun(command: Builtin, value?: string): void;
	} = $props();

	let composer: Composer;
	let slash = $state<SlashMenu>();
	const working = $derived(chat.status !== 'done' && !request);
	const agent = $derived(chat.agent === 'codex' ? 'Codex' : 'Claude');

	function send(text: string, queue: boolean) {
		if (!live || !text.trim()) return false;
		if (request) {
			device.send({ type: 'chat.answer', chatId: chat.id, requestId: request.id, answer: { allow: false, instead: text } });
			return true;
		}
		const [, command, value] = text.trim().match(/^\/(\w+)(?: +(\S+))?$/) ?? [];
		if (command === 'compact' || command === 'clear') {
			if (chat.status !== 'done' || value) return false;
			onrun(command);
			return true;
		}
		if (command === 'model' || command === 'effort' || command === 'mode') {
			if (!choices[command].some((choice) => choice.value === value)) return false;
			onrun(command, value);
			return true;
		}
		device.send({ type: 'chat.send', chatId: chat.id, text, queue });
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

<Composer bind:this={composer} id={chat.id} placeholder={request ? `Or tell ${agent} what to do instead…` : `Message ${chat.rig}`} onkeydown={(event) => slash?.key(event)}>
	{#snippet menu(text, replace)}
		<SlashMenu bind:this={slash} {text} agent={chat.agent} settings={chat.settings} {capabilities} {choices} working={chat.status !== 'done'} {replace} {onrun} />
	{/snippet}
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
