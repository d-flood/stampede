<script lang="ts">
	import { logos } from '#lib/chat/sessions.ts';
	import type { Agent } from '#lib/config.svelte.ts';
	import type { Choice, Setting, Settings } from '#lib/device.svelte.ts';
	import GitBranch from '~icons/ph/git-branch';

	let { rig, agent, settings, choices, project, branch }: { rig: string; agent: Agent; settings: Settings; choices: Record<Setting, Choice[]>; project?: string; branch?: string } = $props();
	const Logo = $derived(logos[agent]);
	const runs = $derived(
		(['model', 'effort', 'mode'] as const).flatMap((setting) => {
			const value = settings[setting];
			return value ? [choices[setting].find((choice) => choice.value === value)?.label ?? value] : [];
		})
	);
	const hints = [
		['/', 'commands'],
		['@', 'files'],
		['!', 'shell'],
		[';', 'Clips']
	];
</script>

<section class="empty" aria-label="New Chat">
	<h2><Logo aria-hidden="true" />{rig}</h2>
	{#if runs.length}<p class="runs">{runs.join(', ')}</p>{/if}
	{#if project}
		<p class="where">
			<span>{project}</span>
			{#if branch}<span class="branch"><GitBranch role="img" aria-label="Branch" />{branch}</span>{/if}
		</p>
	{/if}
	<ul class="hints" aria-label="Input hints">
		{#each hints as [key, label] (key)}<li><kbd>{key}</kbd>{label}</li>{/each}
	</ul>
</section>

<style>
	.empty {
		display: flex;
		flex: 1;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: var(--space-1);
		min-height: 100%;
		color: var(--muted-fg);
		text-align: center;
		font-family: var(--font-ui);
		font-size: var(--font-size-sm);
	}

	h2 {
		display: inline-flex;
		align-items: center;
		gap: var(--space-3);
		margin: 0 0 var(--space-1);
		color: var(--color-fg);
		font: 500 1.5rem var(--font-chat);
		overflow-wrap: anywhere;
	}

	h2 :global(svg) {
		flex: none;
		width: 1.1em;
		height: 1.1em;
	}

	p {
		margin: 0;
		overflow-wrap: anywhere;
	}

	.where {
		display: flex;
		flex-wrap: wrap;
		justify-content: center;
		gap: 0 var(--space-3);
	}

	.branch {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
	}

	.hints {
		display: flex;
		flex-wrap: wrap;
		justify-content: center;
		gap: var(--space-2) var(--space-4);
		margin: var(--space-4) 0 0;
		padding: 0;
		list-style: none;
	}

	kbd {
		margin-right: var(--space-1);
		padding: 1px 6px;
		border: var(--border-width) solid var(--border);
		border-bottom-width: 2px;
		border-radius: var(--radius-sm);
		background: var(--color-bg);
		font: var(--font-size-sm) var(--font-mono);
	}
</style>
