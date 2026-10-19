<script lang="ts">
	import type { Reading } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';
	import { onMount } from 'svelte';
	import Claude from '~icons/simple-icons/claude';
	import OpenAI from '~icons/simple-icons/openai';

	let { readings }: { readings: Reading[] } = $props();

	const agents = [
		{ agent: 'claude-code', label: 'Claude', Logo: Claude },
		{ agent: 'codex', label: 'Codex', Logo: OpenAI }
	] as const;
	const windows = [
		{ window: 'five-hour', label: '5h' },
		{ window: 'weekly', label: 'Weekly' }
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

	function until(resetsAt: number) {
		const minutes = Math.max(0, Math.ceil((resetsAt - now) / 60_000));
		if (minutes === 0) return 'now';
		const days = Math.floor(minutes / (24 * 60));
		const hours = Math.floor((minutes % (24 * 60)) / 60);
		if (days) return `in ${days}d ${hours}h`;
		if (hours) return `in ${hours}h ${minutes % 60}m`;
		return `in ${minutes}m`;
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
	{#each agents as { agent, label, Logo } (agent)}
		{@const updated = Math.max(0, ...readings.filter((reading) => reading.agent === agent).map((reading) => reading.readAt))}
		<div class="agent {agent}" role="group" aria-label={label}>
			<div class="identity">
				<span class="brand" aria-hidden="true"><Logo /></span>
				<div>
					<div class="name">{label}</div>
					<div class="age">{updated ? age(updated) : ''}</div>
				</div>
			</div>
			{#each windows as { window, label: period } (window)}
				{@const reading = readings.find((reading) => reading.agent === agent && reading.window === window)}
				{@const percent = reading ? Math.round(reading.used * 100) : 0}
				{@const description = details(reading)}
				<div class="meter" class:high={reading && reading.used >= 0.9} title={description}>
					<span class="ring" role="meter" aria-label="{label} · {window === 'weekly' ? 'week' : '5h'}" aria-valuemin={0} aria-valuemax={100} aria-valuenow={reading ? percent : undefined} aria-valuetext={reading ? `${percent}% used. ${description}` : description}>
						<svg viewBox="0 0 40 40" aria-hidden="true">
							<circle class="track" cx="20" cy="20" r="17" />
							<circle class="fill" cx="20" cy="20" r="17" pathLength="100" stroke-dasharray="{Math.max(0, Math.min(percent, 100))} 100" transform="rotate(-90 20 20)" />
						</svg>
						<span class="percent">{reading ? `${percent}%` : '—'}</span>
					</span>
					<span class="label">{period}</span>
					<span class="reset">
						{#if reading?.resetsAt !== undefined}
							<time aria-label="Resets {new Date(reading.resetsAt).toLocaleString()}" datetime={new Date(reading.resetsAt).toISOString()}>{new Date(reading.resetsAt).toLocaleString(undefined, { weekday: 'short', hour: 'numeric', minute: '2-digit' })}</time>
							<span class="countdown">({until(reading.resetsAt)})</span>
						{:else}
							Reset unavailable
						{/if}
					</span>
				</div>
			{/each}
		</div>
	{/each}
</section>

<style>
	section {
		display: grid;
		margin-top: auto;
		padding: 0 var(--space-4);
		border-top: var(--border-width) solid var(--border);
		font-size: var(--font-size-sm);
		container-type: inline-size;
	}

	.phone {
		margin-top: 0;
		border-top: none;
		border-bottom: var(--border-width) solid var(--border);
	}

	.agent {
		display: grid;
		grid-template-columns: 82px repeat(2, minmax(0, 1fr));
		align-items: start;
		gap: var(--space-3);
		padding: var(--space-6) 0;
	}

	.agent + .agent {
		border-top: var(--border-width) solid var(--border);
	}

	.claude-code {
		--brand: #d97757;
	}

	.codex {
		--brand: #52669e;
	}

	.identity {
		display: flex;
		align-self: center;
		align-items: center;
		gap: var(--space-2);
		min-width: 0;
	}

	.brand {
		display: flex;
		flex: none;
		color: var(--brand);
	}

	.brand :global(svg) {
		width: 18px;
		height: 18px;
	}

	.name {
		font-size: var(--font-size-md);
		font-weight: 500;
	}

	.meter {
		display: grid;
		grid-template-columns: 32px minmax(0, 1fr);
		align-items: center;
		gap: var(--space-1) var(--space-2);
		min-width: 0;
	}

	.label,
	.reset {
		color: var(--muted-fg);
	}

	.reset {
		display: flex;
		grid-column: 1 / -1;
		flex-wrap: wrap;
		gap: 0 var(--space-1);
		font-size: var(--font-size-sm);
	}

	time,
	.countdown {
		white-space: nowrap;
	}

	.ring {
		display: grid;
		place-items: center;
		width: 32px;
		height: 32px;
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
		stroke: var(--brand);
	}

	.percent,
	.age {
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		font-variant-numeric: tabular-nums;
	}

	.age {
		margin-top: var(--space-1);
		color: var(--muted-fg);
		font-family: var(--font-ui);
		white-space: nowrap;
	}

	.high .fill {
		stroke: var(--ansi-3);
	}

	.high .percent {
		color: var(--ansi-3);
	}

	@media (prefers-color-scheme: dark) {
		.codex {
			--brand: #b7c5ee;
		}
	}

	@container (max-width: 16rem) {
		section:not(.phone) .agent {
			grid-template-columns: 18px repeat(2, minmax(0, 1fr));
		}

		section:not(.phone) .identity > div {
			display: none;
		}
	}

	@container (min-width: 32rem) {
		.agent {
			grid-template-columns: 132px repeat(2, minmax(0, 1fr));
			gap: var(--space-6);
		}

		.identity {
			gap: var(--space-3);
		}

		.brand :global(svg) {
			width: 25px;
			height: 25px;
		}

		.meter {
			grid-template-columns: 44px minmax(0, 1fr);
		}

		.ring {
			grid-row: span 2;
			width: 44px;
			height: 44px;
		}

		.reset {
			grid-column: 2;
		}
	}
</style>
