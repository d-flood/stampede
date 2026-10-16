<script lang="ts">
	import { serverLanguage, type LanguageServers } from '#lib/editor/lsp.svelte.ts';

	type Props = { servers: LanguageServers; projectId: string; path: string; attached: boolean };

	let { servers, projectId, path, attached }: Props = $props();

	const language = $derived(attached ? serverLanguage(path) : undefined);
	const on = $derived(servers.switches[projectId] ?? false);
	const running = $derived(language && servers.states[`lsp:${projectId}:${language}`]);
	const state = $derived(!on ? 'off' : !language ? 'on' : !running || running === 'off' ? 'starting' : running);
	const log = $derived(`/api/lsp/log?${new URLSearchParams({ projectId, language: language ?? '' })}`);
</script>

<button
	class={state}
	role="switch"
	aria-checked={on}
	aria-label="Language server"
	title="Language server: {state}"
	onclick={() => servers.toggle(projectId)}
>
	<span class="dot"></span>LSP {state}
</button>
{#if state === 'error'}
	<a href={log} target="_blank" rel="noopener">view log</a>
{/if}

<style>
	button {
		display: inline-flex;
		flex: none;
		align-items: center;
		gap: var(--space-2);
		min-height: var(--target-min);
		padding: 0 var(--space-2);
		border: none;
		border-radius: var(--radius-sm);
		background: none;
		color: var(--muted-fg);
		font: inherit;
		font-size: var(--font-size-sm);
		cursor: pointer;
		transition:
			background var(--dur-fast),
			color var(--dur-fast);
	}

	button:hover {
		background: var(--hover);
		color: var(--color-fg);
	}

	button:focus-visible {
		outline: var(--border-width) solid var(--status-working);
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
