<script lang="ts">
	import CheckBold from '~icons/ph/check-bold';
	import FileCode from '~icons/ph/file-code';
	import GitDiff from '~icons/ph/git-diff';
	import TerminalWindow from '~icons/ph/terminal-window';
	import Claude from '~icons/simple-icons/claude';
	import OpenAI from '~icons/simple-icons/openai';
	import type { TabProps } from '../data.ts';

	let { variant, label, agent = 'claude-code', status, active = false, added, removed }: TabProps = $props();

	const icons = { terminal: TerminalWindow, editor: FileCode, diff: GitDiff };
	const agents = { 'claude-code': Claude, codex: OpenAI };
	const Icon = $derived(variant === 'agent' ? agents[agent] : icons[variant]);
</script>

<button role="tab" aria-selected={active} class="{variant} {status ?? ''}" class:active>
	{#if variant === 'agent'}
		<span class="avatar" title={status}>
			<Icon />
			{#if status === 'done'}<span class="check"><CheckBold /></span>{/if}
		</span>
	{:else}
		<Icon />
	{/if}
	<span class="label">{label}</span>
	{#if variant === 'diff'}
		<span class="counts"><span class="added">+{added}</span> <span class="removed">−{removed}</span></span>
	{/if}
</button>

<style>
	button {
		display: inline-flex;
		flex: none;
		align-items: center;
		gap: var(--space-2);
		min-width: 0;
		max-width: 14rem;
		min-height: var(--target-min);
		margin: var(--space-1) 0 var(--space-1) var(--space-1);
		padding: 0 var(--space-3);
		border: none;
		border-radius: var(--radius-md);
		background: none;
		color: var(--muted-fg);
		font: inherit;
		font-size: var(--font-size-sm);
		white-space: nowrap;
		cursor: pointer;
	}

	.label {
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.active {
		background: var(--surface-3);
		color: var(--color-fg);
	}

	.agent {
		padding-left: var(--space-1);
		border-radius: calc(var(--target-min) / 2);
		color: var(--color-fg);
		font-weight: 600;
	}

	.terminal .label {
		font-family: var(--font-mono);
	}

	.avatar {
		position: relative;
		display: grid;
		flex: none;
		place-items: center;
		width: calc(var(--target-min) - 2 * var(--space-1));
		aspect-ratio: 1;
		border-radius: 50%;
		background: var(--color-bg);
		box-shadow: inset 0 0 0 2px var(--ring, var(--border));
	}

	.working .avatar::after {
		content: '';
		position: absolute;
		inset: 0;
		border: 2px solid var(--status-working);
		border-right-color: transparent;
		border-bottom-color: transparent;
		border-radius: 50%;
		animation: spin 1s linear var(--pulse);
	}

	.waiting {
		background: color-mix(in oklab, var(--status-waiting) 22%, transparent);
	}

	.waiting.active {
		background: color-mix(in oklab, var(--status-waiting) 30%, var(--surface-3));
	}

	.waiting .avatar {
		--ring: var(--status-waiting);
		animation: fade 1.6s ease-in-out var(--pulse);
	}

	.done .avatar {
		--ring: var(--status-done);
	}

	.check {
		position: absolute;
		right: -0.3em;
		bottom: -0.3em;
		display: grid;
		place-items: center;
		width: 1.1em;
		height: 1.1em;
		border-radius: 50%;
		background: var(--status-done);
		color: var(--color-bg);
		font-size: 0.7em;
	}

	.counts {
		font-family: var(--font-mono);
		font-weight: 400;
	}

	.added {
		color: var(--ansi-2);
	}

	.removed {
		color: var(--ansi-1);
	}

	@keyframes spin {
		to {
			transform: rotate(1turn);
		}
	}

	@keyframes fade {
		50% {
			box-shadow: inset 0 0 0 2px transparent;
		}
	}
</style>
