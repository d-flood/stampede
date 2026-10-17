<script lang="ts">
	import type { Reading } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';
	import { onMount } from 'svelte';

	let { readings }: { readings: Reading[] } = $props();

	const meters = [
		{ agent: 'claude-code', window: 'five-hour', label: 'Claude · 5h' },
		{ agent: 'claude-code', window: 'weekly', label: 'Claude · week' },
		{ agent: 'codex', window: 'five-hour', label: 'Codex · 5h' },
		{ agent: 'codex', window: 'weekly', label: 'Codex · week' }
	] as const;

	let now = $state(Date.now());

	onMount(() => {
		const timer = setInterval(() => (now = Date.now()), 30_000);
		return () => clearInterval(timer);
	});

	function age(readAt: number) {
		const minutes = Math.floor((now - readAt) / 60_000);
		if (minutes < 1) return 'just now';
		if (minutes < 60) return `${minutes}m ago`;
		if (minutes < 24 * 60) return `${Math.floor(minutes / 60)}h ago`;
		return `${Math.floor(minutes / (24 * 60))}d ago`;
	}

	function details(reading: Reading | undefined) {
		if (!reading) return 'No usage reading available';
		const updated = `Last updated: ${new Date(reading.readAt).toLocaleString()}`;
		const reset = reading.resetsAt === undefined
			? 'Reset time unavailable'
			: `Resets: ${new Date(reading.resetsAt).toLocaleString()}`;
		return `${updated}\n${reset}`;
	}
</script>

<section aria-label="Usage windows" class:phone={form() === 'phone'}>
	{#each meters as { agent, window, label } (label)}
		{@const reading = readings.find((reading) => reading.agent === agent && reading.window === window)}
		{@const percent = reading ? Math.round(reading.used * 100) : 0}
		{@const description = details(reading)}
		<div class="meter" class:high={reading && reading.used >= 0.9} title={description}>
			<span class="label">{label}</span>
			<span class="bar" role="meter" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent} aria-valuetext={reading ? `${percent}% used. ${description}` : description}>
				<span class="fill" style:width="{Math.min(percent, 100)}%"></span>
			</span>
			<span class="percent">{reading ? `${percent}%` : '—'}</span>
			<span class="age">
				{reading ? age(reading.readAt) : ''}
			</span>
		</div>
	{/each}
</section>

<style>
	section {
		display: grid;
		grid-template-columns: max-content 1fr max-content max-content;
		align-items: center;
		gap: var(--space-2) var(--space-3);
		margin-top: auto;
		padding: var(--space-3) var(--space-4) var(--space-4);
		border-top: var(--border-width) solid var(--border);
		font-size: var(--font-size-sm);
	}

	.phone {
		margin-top: 0;
		border-top: none;
		border-bottom: var(--border-width) solid var(--border);
		font-size: var(--font-size-md);
	}

	.meter {
		display: contents;
	}

	.label {
		color: var(--muted-fg);
	}

	.bar {
		height: 4px;
		border-radius: 2px;
		background: var(--surface-3);
	}

	.phone .bar {
		height: 5px;
	}

	.fill {
		display: block;
		height: 100%;
		border-radius: inherit;
		background: var(--ansi-4);
	}

	.percent,
	.age {
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		font-variant-numeric: tabular-nums;
		text-align: end;
	}

	.age {
		color: var(--muted-fg);
	}

	.high .fill {
		background: var(--ansi-3);
	}

	.high .percent {
		color: var(--ansi-3);
	}
</style>
