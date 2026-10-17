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
			<span class="ring" role="meter" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={reading ? percent : undefined} aria-valuetext={reading ? `${percent}% used. ${description}` : description}>
				<svg viewBox="0 0 40 40" aria-hidden="true">
					<circle class="track" cx="20" cy="20" r="17" />
					<circle class="fill" cx="20" cy="20" r="17" pathLength="100" stroke-dasharray="{Math.max(0, Math.min(percent, 100))} 100" transform="rotate(-90 20 20)" />
				</svg>
				<span class="percent">{reading ? `${percent}%` : '—'}</span>
			</span>
			<span class="label">{label}</span>
			<span class="age">
				{reading ? age(reading.readAt) : ''}
			</span>
			<span class="reset">
				{#if reading?.resetsAt !== undefined}
					Resets <time datetime={new Date(reading.resetsAt).toISOString()}>{new Date(reading.resetsAt).toLocaleString(undefined, { weekday: 'short', hour: 'numeric', minute: '2-digit' })}</time>
				{:else}
					Reset time unavailable
				{/if}
			</span>
		</div>
	{/each}
</section>

<style>
	section {
		display: grid;
		gap: var(--space-3);
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
		display: grid;
		grid-template-columns: 40px minmax(0, 1fr) max-content;
		align-items: center;
		column-gap: var(--space-3);
	}

	.label,
	.reset {
		color: var(--muted-fg);
	}

	.reset {
		grid-column: 2 / -1;
	}

	.ring {
		display: grid;
		grid-row: span 2;
		place-items: center;
		width: 40px;
		height: 40px;
	}

	.ring svg,
	.percent {
		grid-area: 1 / 1;
	}

	.ring svg {
		width: 100%;
		height: 100%;
		fill: none;
		stroke-width: 3;
	}

	.track {
		stroke: var(--surface-3);
	}

	.fill {
		stroke: var(--ansi-4);
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
		stroke: var(--ansi-3);
	}

	.high .percent {
		color: var(--ansi-3);
	}
</style>
