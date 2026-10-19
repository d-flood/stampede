<script lang="ts">
	import type { Reading } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';
	import Tooltip from '#lib/ui/Tooltip.svelte';
	import { onMount } from 'svelte';
	import Claude from '~icons/simple-icons/claude';
	import OpenAI from '~icons/simple-icons/openai';
	import Copilot from '~icons/simple-icons/githubcopilot';

	let { readings }: { readings: Reading[] } = $props();

	const windows = [
		{ window: 'five-hour', label: '5h', name: '5h' },
		{ window: 'weekly', label: 'Weekly', name: 'week' }
	] as const;
	const agents = [
		{ agent: 'claude-code', label: 'Claude', Logo: Claude },
		{ agent: 'codex', label: 'Codex', Logo: OpenAI },
		{ agent: 'copilot', label: 'Copilot', Logo: Copilot }
	] as const;

	const visibleAgents = $derived(agents.filter(({ agent }) => readings.some((reading) => reading.agent === agent && (agent === 'copilot' || reading.used !== undefined))));

	let now = $state(Date.now());
	let section: HTMLElement;
	let compact = $state(false);

	onMount(() => {
		const timer = setInterval(() => (now = Date.now()), 30_000);
		const observer = new ResizeObserver(() => {
			compact = getComputedStyle(section.querySelector('.agent') ?? section).getPropertyValue('--compact').trim() === '1';
		});
		observer.observe(section);
		return () => {
			clearInterval(timer);
			observer.disconnect();
		};
	});

	function resetTime(resetsAt: number) {
		return new Date(resetsAt).toLocaleString(undefined, { weekday: 'short', hour: 'numeric', minute: '2-digit' });
	}

	function periodDate(at: number) {
		return new Date(at).toLocaleDateString(undefined, { month: 'short', day: 'numeric', timeZone: 'UTC' });
	}

	function cost(credits: number) {
		const dollars = credits / 100;
		if (dollars > 0 && dollars < 0.0001) return '<$0.0001';
		return dollars.toLocaleString(undefined, { style: 'currency', currency: 'USD', maximumFractionDigits: 4 });
	}

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
		if (!reading) return '';
		const updated = `Last updated: ${new Date(reading.readAt).toLocaleString()}`;
		const reset = reading.resetsAt === undefined
			? ''
			: `Resets: ${new Date(reading.resetsAt).toLocaleString()}`;
		return [updated, reset].filter(Boolean).join('\n');
	}
</script>

<section bind:this={section} aria-label="Usage windows" class:phone={form() === 'phone'}>
	{#each visibleAgents as { agent, label, Logo } (agent)}
		{@const updated = Math.max(0, ...readings.filter((reading) => reading.agent === agent).map((reading) => reading.readAt))}
		<div class="agent {agent}" role="group" aria-label={label}>
			<div class="identity">
				<span class="brand" aria-hidden="true"><Logo /></span>
				<div>
					<div class="name">{label}</div>
					<div class="age">{updated ? age(updated) : ''}</div>
				</div>
			</div>
			{#if agent === 'copilot'}
				{@const reading = readings.find((reading) => reading.agent === agent && reading.window === 'monthly')}
				{@const currentSessions = readings.filter((reading) => reading.agent === agent && reading.window === 'session')}
				{@const unit = currentSessions.some((reading) => reading.consumption?.unit === 'credits') ? 'credits' : 'requests'}
				{@const sessions = currentSessions.filter((reading) => reading.consumption?.unit === unit)}
				{@const consumption = reading?.consumption ?? (sessions.length ? { amount: sessions.reduce((total, reading) => total + reading.consumption!.amount, 0), unit } : undefined)}
				{@const partial = !reading?.consumption && (sessions.some((reading) => reading.partial) || sessions.length < currentSessions.length)}
				<div class="billing">
					<span class="amount">
						{#if consumption}
							{#if consumption.unit === 'credits'}
								{cost(consumption.amount)}
								{#if partial}<span class="unit">reported</span>{/if}
							{:else}
								{consumption.amount.toLocaleString(undefined, { maximumFractionDigits: 2 })}
								<span class="unit">{consumption.unit}</span>
							{/if}
						{:else}
							Usage unavailable
						{/if}
					</span>
					{#if consumption?.unit === 'credits'}
						<span class="credits">{consumption.amount.toLocaleString(undefined, { maximumFractionDigits: 4 })} credits</span>
					{/if}
					<span class="period">{reading?.consumption ? 'This billing period' : sessions.length ? 'Recorded sessions' : 'Billing usage not reported'}</span>
					{#if !reading?.consumption && consumption?.unit === 'requests'}<span class="cost-unavailable">Cost not reported</span>{/if}
					{#if reading?.consumption && reading.startsAt !== undefined && reading.resetsAt !== undefined}
						<span class="cycle">
							<time datetime={new Date(reading.startsAt).toISOString()}>{periodDate(reading.startsAt)}</time>
							–
							<time datetime={new Date(reading.resetsAt).toISOString()}>{periodDate(reading.resetsAt)}</time>
						</span>
					{/if}
				</div>
			{:else}
				{#each windows as { window, label: period, name } (window)}
					{@const reading = readings.find((reading) => reading.agent === agent && reading.window === window)}
					{@const used = reading?.used}
					{#if used !== undefined}
						{@const percent = Math.round(used * 100)}
						{@const description = details(reading)}
						{@const reset = reading?.resetsAt === undefined ? '' : `${resetTime(reading.resetsAt)} (${until(reading.resetsAt)})`}
						<div class="meter" style:grid-column={window === 'weekly' ? 3 : 2} class:high={used >= 0.9}>
							<Tooltip delayDuration={0} text={reset} disabled={!compact || !reset}>
								{#snippet children(props)}
									<span {...props} class="ring" role="meter" aria-label="{label} · {name}" aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent} aria-valuetext={`${percent}% used. ${description}`}>
										<svg viewBox="0 0 40 40" aria-hidden="true">
											<circle class="track" cx="20" cy="20" r="17" />
											<circle class="fill" cx="20" cy="20" r="17" pathLength="100" stroke-dasharray="{Math.max(0, Math.min(percent, 100))} 100" transform="rotate(-90 20 20)" />
										</svg>
										<span class="percent">{percent}%</span>
									</span>
								{/snippet}
							</Tooltip>
							<span class="label">{period}</span>
							{#if reading?.resetsAt !== undefined}
								<span class="reset">
									<time aria-label="Resets {new Date(reading.resetsAt).toLocaleString()}" datetime={new Date(reading.resetsAt).toISOString()}>{resetTime(reading.resetsAt)}</time>
									<span class="countdown">({until(reading.resetsAt)})</span>
								</span>
							{/if}
						</div>
					{/if}
				{/each}
			{/if}
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

	section:not(:has(.agent)) {
		display: none;
	}

	.phone {
		margin-top: 0;
		border-top: none;
		border-bottom: var(--border-width) solid var(--border);
	}

	.agent {
		--compact: 0;
		display: grid;
		grid-template-columns: max-content repeat(2, minmax(0, 1fr));
		align-items: start;
		gap: var(--space-2);
		padding: var(--space-3) 0;
	}

	.agent + .agent {
		border-top: var(--border-width) solid var(--border);
	}

	.billing {
		display: grid;
		grid-column: span 2;
		gap: var(--space-1);
		min-width: 0;
	}

	.amount {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 0 var(--space-1);
		font-family: var(--font-mono);
		font-size: var(--font-size-md);
		font-variant-numeric: tabular-nums;
	}

	.unit,
	.credits,
	.cost-unavailable,
	.period,
	.cycle {
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.unit {
		font-family: var(--font-ui);
	}

	.claude-code {
		--brand: #d97757;
	}

	.codex {
		--brand: #52669e;
	}

	.copilot {
		--brand: var(--color-fg);
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

		section:not(.phone) .identity > div,
		section:not(.phone) .label,
		section:not(.phone) .reset {
			display: none;
		}

		section:not(.phone) .meter {
			grid-template-columns: 32px;
			justify-content: center;
		}
	}

	@container (max-width: 24rem) {
		.agent {
			--compact: 1;
		}

		.reset time {
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
			align-self: start;
			width: 44px;
			height: 44px;
		}

		.reset {
			grid-column: 2;
		}
	}
</style>
