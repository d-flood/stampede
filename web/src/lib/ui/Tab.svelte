<script lang="ts">
	import type { HTMLButtonAttributes } from 'svelte/elements';
	import FileCode from '~icons/ph/file-code';
	import GitDiff from '~icons/ph/git-diff';
	import TerminalWindow from '~icons/ph/terminal-window';
	import Claude from '~icons/simple-icons/claude';
	import OpenAI from '~icons/simple-icons/openai';

	type Props = HTMLButtonAttributes & {
		variant: 'agent' | 'terminal' | 'editor' | 'diff';
		label: string;
		agent?: 'claude-code' | 'codex';
		status?: 'working' | 'waiting' | 'done';
		active?: boolean;
	};

	let { variant, label, agent = 'claude-code', status, active = false, ...rest }: Props = $props();

	const icons = { terminal: TerminalWindow, editor: FileCode, diff: GitDiff };
	const agents = { 'claude-code': Claude, codex: OpenAI };
	const Icon = $derived(variant === 'agent' ? agents[agent] : icons[variant]);
</script>

<button role="tab" aria-selected={active} class={variant} class:active {...rest}>
	<Icon />
	<span>{label}</span>
	{#if status}
		<span class="status {status}" title={status}></span>
	{/if}
</button>

<style>
	button {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		min-height: var(--target-min);
		padding: 0 var(--space-3);
		border: var(--border-width) solid transparent;
		border-radius: var(--radius-sm) var(--radius-sm) 0 0;
		background: var(--surface-1);
		color: var(--muted-fg);
		font: inherit;
		font-size: var(--font-size-sm);
		white-space: nowrap;
		cursor: pointer;
		transition:
			background var(--dur-fast),
			color var(--dur-fast);
	}

	button:hover {
		background: var(--hover);
		color: var(--color-fg);
	}

	.active {
		border-color: var(--border);
		border-bottom-color: var(--color-bg);
		background: var(--color-bg);
		color: var(--color-fg);
	}

	.agent {
		font-weight: 600;
	}

	.diff {
		font-style: italic;
	}

	.status {
		width: 0.6em;
		height: 0.6em;
		border-radius: 50%;
	}

	.working {
		background: var(--status-working);
	}

	.waiting {
		background: var(--status-waiting);
		animation: fade 1.6s ease-in-out var(--pulse);
	}

	.done {
		background: var(--status-done);
	}

	@keyframes fade {
		50% {
			opacity: 0.25;
		}
	}
</style>
