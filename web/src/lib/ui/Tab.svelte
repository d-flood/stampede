<script lang="ts">
	import type { HTMLButtonAttributes } from 'svelte/elements';
	import Article from '~icons/ph/article';
	import CheckBold from '~icons/ph/check-bold';
	import CircleNotchBold from '~icons/ph/circle-notch-bold';
	import FileCode from '~icons/ph/file-code';
	import Globe from '~icons/ph/globe';
	import GitDiff from '~icons/ph/git-diff';
	import HandPalmFill from '~icons/ph/hand-palm-fill';
	import TerminalWindow from '~icons/ph/terminal-window';
	import X from '~icons/ph/x';
	import Claude from '~icons/simple-icons/claude';
	import OpenAI from '~icons/simple-icons/openai';

	type Props = HTMLButtonAttributes & {
		variant: 'agent' | 'terminal' | 'editor' | 'diff' | 'browser' | 'epic';
		label: string;
		agent?: 'claude-code' | 'codex';
		against?: string;
		status?: 'working' | 'waiting' | 'done';
		active?: boolean;
		dirty?: boolean;
		deleted?: boolean;
		ended?: boolean;
		onclose?: () => void;
	};

	let {
		variant,
		label,
		agent = 'claude-code',
		against = 'HEAD',
		status,
		active = false,
		dirty = false,
		deleted = false,
		ended = false,
		onclose,
		...rest
	}: Props = $props();

	const icons = { terminal: TerminalWindow, editor: FileCode, diff: GitDiff, browser: Globe, epic: Article };
	const agents = { 'claude-code': Claude, codex: OpenAI };
	const marks = { working: CircleNotchBold, waiting: HandPalmFill, done: CheckBold };
	const Icon = $derived(variant === 'agent' ? agents[agent] : icons[variant]);
	const Mark = $derived(status && marks[status]);
</script>

<div class="tab {status ?? ''}" class:closable={active && !!onclose}>
	<button role="tab" aria-selected={active} class="select {variant} {status ?? ''}" class:active class:deleted class:ended {...rest}>
		<Icon />
		<span class="label">{label}</span>
		{#if variant === 'diff'}<span class="vs">↔ {against}</span>{/if}
		{#if Mark}<span class="mark" title={status}><Mark /></span>{/if}
		{#if dirty}<span class="dirty" title="Unsaved changes"></span>{/if}
	</button>
	{#if active && onclose}
		<button
			class="close"
			type="button"
			aria-label="Close {label}"
			title="Close {label} (Ctrl/Cmd+W)"
			onpointerdown={(event) => event.stopPropagation()}
			onclick={(event) => {
				event.stopPropagation();
				onclose?.();
			}}
		><X /></button>
	{/if}
</div>

<style>
	.tab {
		position: relative;
		display: inline-flex;
		flex: none;
		min-width: 0;
		max-width: 14rem;
	}

	.select {
		position: relative;
		display: inline-flex;
		flex: 1;
		align-items: center;
		gap: var(--space-2);
		min-width: 0;
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

	.select:not(.active):hover {
		background: var(--hover);
		color: var(--color-fg);
	}

	.closable .select {
		padding-right: calc(var(--target-min) + var(--space-3));
	}

	.close {
		position: absolute;
		top: 50%;
		right: var(--border-width);
		display: inline-grid;
		place-items: center;
		width: var(--target-min);
		height: var(--target-min);
		padding: 0;
		border: none;
		border-radius: var(--radius-sm);
		background: none;
		color: var(--muted-fg);
		font-size: var(--font-size-lg);
		cursor: pointer;
		transform: translateY(-50%);
	}

	.close:hover {
		background: var(--hover);
		color: var(--color-fg);
	}

	.close:focus-visible {
		outline: var(--border-width) solid var(--status-working);
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

	.deleted .label {
		text-decoration: line-through;
	}

	.ended {
		color: var(--muted-fg);
		font-weight: 400;
		font-style: italic;
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

	.dirty {
		flex: none;
		width: 0.5em;
		height: 0.5em;
		border-radius: 50%;
		background: currentColor;
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
