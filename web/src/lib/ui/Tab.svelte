<script lang="ts">
	import type { HTMLButtonAttributes } from 'svelte/elements';
	import CheckBold from '~icons/ph/check-bold';
	import CircleNotchBold from '~icons/ph/circle-notch-bold';
	import FileCode from '~icons/ph/file-code';
	import GitDiff from '~icons/ph/git-diff';
	import HandPalmFill from '~icons/ph/hand-palm-fill';
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
	const marks = { working: CircleNotchBold, waiting: HandPalmFill, done: CheckBold };
	const Icon = $derived(variant === 'agent' ? agents[agent] : icons[variant]);
	const Mark = $derived(status && marks[status]);
</script>

<button role="tab" aria-selected={active} class="{variant} {status ?? ''}" class:active {...rest}>
	<Icon />
	<span class="label">{label}</span>
	{#if variant === 'diff'}<span class="vs">↔ HEAD</span>{/if}
	{#if Mark}<span class="mark" title={status}><Mark /></span>{/if}
</button>

<style>
	button {
		position: relative;
		display: inline-flex;
		flex: none;
		align-items: center;
		gap: var(--space-2);
		min-width: 0;
		max-width: 14rem;
		min-height: calc(var(--target-min) + var(--space-2));
		margin-bottom: calc(-1 * var(--border-width));
		padding: 0 var(--space-3);
		border: none;
		border-right: var(--border-width) solid var(--border);
		border-bottom: var(--border-width) solid var(--border);
		background: none;
		color: var(--muted-fg);
		font: inherit;
		font-size: var(--font-size-sm);
		white-space: nowrap;
		cursor: pointer;
		transition:
			background var(--dur-fast),
			color var(--dur-fast);
	}

	button:not(.active):hover {
		background: var(--hover);
		color: var(--color-fg);
	}

	.label {
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.active {
		border-bottom-color: var(--color-bg);
		background: var(--color-bg);
		color: var(--color-fg);
	}

	.active::before {
		content: '';
		position: absolute;
		top: 0;
		right: 0;
		left: 0;
		height: 2px;
		background: var(--color-fg);
	}

	.agent {
		color: var(--color-fg);
		font-weight: 600;
	}

	.agent::after {
		content: '';
		position: absolute;
		right: 0;
		bottom: 0;
		left: 0;
		height: 2px;
		background: var(--mark);
	}

	.terminal .label {
		font-family: var(--font-mono);
	}

	.vs {
		color: var(--muted-fg);
		font-weight: 400;
	}

	.working {
		--mark: var(--status-working);
	}

	.waiting {
		--mark: var(--status-waiting);
		background: color-mix(in oklab, var(--status-waiting) 16%, transparent);
	}

	.done {
		--mark: var(--status-done);
	}

	.mark {
		display: inline-flex;
		color: var(--mark);
	}

	.working .mark {
		animation: spin 1s linear var(--pulse);
	}

	.waiting .mark {
		animation: fade 1.6s ease-in-out var(--pulse);
	}

	@keyframes spin {
		to {
			transform: rotate(1turn);
		}
	}

	@keyframes fade {
		50% {
			opacity: 0.3;
		}
	}
</style>
