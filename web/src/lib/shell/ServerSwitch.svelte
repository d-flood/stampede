<script lang="ts">
	import { serverLanguages, type LanguageServers } from '#lib/editor/lsp.svelte.ts';
	import Button from '#lib/ui/Button.svelte';

	type Props = { servers: LanguageServers; projectId: string; path: string };

	let { servers, projectId, path }: Props = $props();

	const languages = $derived(serverLanguages(path));
	const on = $derived(servers.switches[projectId] ?? false);
	const states = $derived(languages.map((language) => servers.states[`lsp:${projectId}:${language}`]));
	const failed = $derived(languages.find((_, index) => states[index] === 'error'));
	const running = $derived(states[0]);
	const state = $derived(
		!on ? 'off' : !languages.length ? 'on' : failed ? 'error' : !running || running === 'off' ? 'starting' : running
	);
	const log = $derived(`/api/lsp/log?${new URLSearchParams({ projectId, language: failed ?? languages[0] ?? '' })}`);
</script>

<Button
	variant="ghost"
	size="sm"
	role="switch"
	aria-checked={on}
	aria-label="Language server"
	title="Language server: {state}"
	onclick={() => servers.toggle(projectId)}
>
	<span class="state {state}"><span class="dot"></span>LSP {state}</span>
</Button>
{#if state === 'error'}
	<a href={log} target="_blank" rel="noopener">view log</a>
{/if}

<style>



	.state {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
	}

	.dot {
		width: 0.6em;
		height: 0.6em;
		box-sizing: border-box;
		border: var(--border-width) solid currentColor;
		border-radius: 50%;
	}

	.starting .dot {
		border-color: var(--status-working);
		background: var(--status-working);
		animation: fade 1.6s ease-in-out var(--pulse);
	}

	.on .dot {
		border-color: var(--status-done);
		background: var(--status-done);
	}

	.error {
		color: var(--ansi-1);
	}

	.error .dot {
		background: currentColor;
	}

	a {
		display: inline-flex;
		flex: none;
		align-items: center;
		min-height: var(--target-min);
		color: var(--ansi-1);
		font-size: var(--font-size-sm);
	}

	@keyframes fade {
		50% {
			opacity: 0.3;
		}
	}
</style>
