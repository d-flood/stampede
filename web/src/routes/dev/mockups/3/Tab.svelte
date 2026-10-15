<script lang="ts">
	import FileCode from '~icons/ph/file-code';
	import GitDiff from '~icons/ph/git-diff';
	import TerminalWindow from '~icons/ph/terminal-window';
	import Claude from '~icons/simple-icons/claude';
	import OpenAI from '~icons/simple-icons/openai';
	import type { TabProps } from '../data.ts';

	let { variant, label, agent = 'claude-code', status, active = false, added, removed }: TabProps = $props();

	const icons = { terminal: TerminalWindow, editor: FileCode, diff: GitDiff };
	const agents = { 'claude-code': Claude, codex: OpenAI };
	const words = { working: 'Working', waiting: 'Waiting on you', done: 'Done' };
	const Icon = $derived(variant === 'agent' ? agents[agent] : icons[variant]);
	const caption = $derived(
		status ? words[status] : { agent: 'Agent', terminal: 'Terminal', editor: 'Editor', diff: 'Diff vs HEAD' }[variant]
	);
</script>

<button role="tab" aria-selected={active} class="{variant} {status ?? ''}" class:active>
	<span class="text">
		<span class="label">{label}</span>
		<span class="caption">
			{caption}
			{#if variant === 'diff'}<span class="added">+{added}</span> <span class="removed">−{removed}</span>{/if}
		</span>
	</span>
	<span class="icon"><Icon /></span>
</button>

<style>
	button {
		position: relative;
		display: inline-flex;
		flex: none;
		align-items: center;
		gap: var(--space-3);
		min-width: 0;
		max-width: 15rem;
		min-height: var(--target-min);
		margin-bottom: calc(-1 * var(--border-width));
		padding: var(--space-1) var(--space-3) var(--space-1) calc(var(--space-3) + 4px);
		border: none;
		border-right: var(--border-width) solid var(--border);
		border-bottom: var(--border-width) solid var(--border);
		background: none;
		color: var(--muted-fg);
		font: inherit;
		text-align: left;
		white-space: nowrap;
		cursor: pointer;
	}

	.text {
		display: flex;
		flex-direction: column;
		min-width: 0;
		line-height: 1.25;
	}

	.label {
		overflow: hidden;
		font-size: var(--font-size-md);
		font-weight: 600;
		text-overflow: ellipsis;
	}

	.caption {
		font-size: var(--font-size-sm);
	}

	.icon {
		display: inline-flex;
		font-size: var(--font-size-lg);
	}

	.active {
		border-bottom-color: var(--color-bg);
		background: var(--color-bg);
		color: var(--color-fg);
	}

	.agent {
		color: var(--color-fg);
	}

	.agent::before {
		content: '';
		position: absolute;
		top: 0;
		bottom: 0;
		left: 0;
		width: 4px;
		background: var(--mark);
	}

	.terminal .label {
		font-family: var(--font-mono);
		font-weight: 400;
	}

	.agent .caption {
		color: var(--mark);
		font-weight: 600;
	}

	.working {
		--mark: var(--status-working);
	}

	.waiting {
		--mark: var(--status-waiting);
		background: color-mix(in oklab, var(--status-waiting) 16%, transparent);
	}

	.waiting::before {
		animation: fade 1.6s ease-in-out var(--pulse);
	}

	.done {
		--mark: var(--status-done);
	}

	.added {
		color: var(--ansi-2);
	}

	.removed {
		color: var(--ansi-1);
	}

	@keyframes fade {
		50% {
			opacity: 0.3;
		}
	}
</style>
