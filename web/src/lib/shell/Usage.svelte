<script lang="ts">
	import { logos } from '#lib/chat/sessions.ts';
	import type { Reading, ResetCredits } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';
	import CodexResets, { expiry } from '#lib/shell/CodexResets.svelte';
	import Tooltip from '#lib/ui/Tooltip.svelte';
	import { mergeProps, Popover } from 'bits-ui';
	import { onMount } from 'svelte';
	import ResetIcon from '~icons/ph/arrow-counter-clockwise-bold';

	let { readings, codexResets = null }: { readings: Reading[]; codexResets?: ResetCredits | null } = $props();

	const windows = [
		{ window: 'five-hour', label: '5h', name: '5h', short: '5h' },
		{ window: 'weekly', label: 'Weekly', name: 'week', short: 'wk' }
	] as const;
	const agents = [
		{ agent: 'claude-code', label: 'Claude', Logo: logos['claude-code'] },
		{ agent: 'codex', label: 'Codex', Logo: logos.codex },
		{ agent: 'copilot', label: 'Copilot', Logo: logos.copilot }
	] as const;
	const high = 80;

	const visibleAgents = $derived(agents.filter(({ agent }) => (agent === 'codex' && codexResets !== null) || readings.some((reading) => reading.agent === agent && (agent === 'copilot' || reading.used !== undefined))));
	const resets = $derived(codexResets?.availableCount ?? 0);
	const crowded = $derived(agents.flatMap(({ agent }) => measured(agent)).length > 3);

	let now = $state(Date.now());
	let open = $state(false);
	let tip = $state(false);
	let popover = $state<HTMLElement>();

	onMount(() => {
		const timer = setInterval(() => (now = Date.now()), 30_000);
		return () => clearInterval(timer);
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

	function remaining(resetsAt: number) {
		const minutes = Math.max(0, Math.ceil((resetsAt - now) / 60_000));
		if (minutes === 0) return 'now';
		const days = Math.floor(minutes / (24 * 60));
		const hours = Math.floor((minutes % (24 * 60)) / 60);
		if (days) return `${days}d ${hours}h`;
		if (hours) return `${hours}h ${minutes % 60}m`;
		return `${minutes}m`;
	}

	function until(resetsAt: number) {
		const left = remaining(resetsAt);
		return left === 'now' ? left : `in ${left}`;
	}

	function details(reading: Reading | undefined) {
		if (!reading) return '';
		const updated = `Last updated: ${new Date(reading.readAt).toLocaleString()}`;
		const reset = reading.resetsAt === undefined
			? ''
			: `Resets: ${new Date(reading.resetsAt).toLocaleString()}`;
		return [updated, reset].filter(Boolean).join('\n');
	}

	function measured(agent: string) {
		return windows.flatMap(({ window, label, name, short }) => {
			const reading = readings.find((reading) => reading.agent === agent && reading.window === window);
			return reading?.used === undefined ? [] : [{ window, label, name, short, reading, percent: Math.round(reading.used * 100) }];
		});
	}
</script>

{#snippet billing()}
	{@const reading = readings.find((reading) => reading.agent === 'copilot' && reading.window === 'monthly')}
	{@const currentSessions = readings.filter((reading) => reading.agent === 'copilot' && reading.window === 'session')}
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
		{#if reading?.consumption || !sessions.length}
			<span class="period">{reading?.consumption ? 'This billing period' : 'Billing usage not reported'}</span>
		{/if}
		{#if !reading?.consumption && consumption?.unit === 'requests'}<span class="period">Cost not reported</span>{/if}
		{#if reading?.consumption && reading.startsAt !== undefined && reading.resetsAt !== undefined}
			<span class="cycle">
				<time datetime={new Date(reading.startsAt).toISOString()}>{periodDate(reading.startsAt)}</time>
				–
				<time datetime={new Date(reading.resetsAt).toISOString()}>{periodDate(reading.resetsAt)}</time>
			</span>
		{/if}
	</div>
{/snippet}

{#snippet detail(popover: boolean)}
	<div class="detail">
		{#each visibleAgents as { agent, label, Logo } (agent)}
			{@const updated = Math.max(0, ...readings.filter((reading) => reading.agent === agent).map((reading) => reading.readAt))}
			<div class="agent-detail {agent}">
				<div class="heading">
					<span class="brand" aria-hidden="true"><Logo /></span>
					<strong>{label}</strong>
					{#if updated}<span class="age">{age(updated)}</span>{/if}
				</div>
				{#if agent === 'copilot'}
					{@render billing()}
				{:else}
					{#each measured(agent) as { window, label: period, reading, percent } (window)}
						<div class="window" class:high={percent >= high}>
							<span class="period">{period}</span>
							<span class="bar" aria-hidden="true"><span class="fill" style:width="{Math.min(percent, 100)}%"></span></span>
							<span class="percent">{percent}%</span>
							{#if reading.resetsAt !== undefined}
								<span class="reset">
									<time aria-label="Resets {new Date(reading.resetsAt).toLocaleString()}" datetime={new Date(reading.resetsAt).toISOString()}>{resetTime(reading.resetsAt)}</time>
									<span class="countdown">{until(reading.resetsAt)}</span>
								</span>
							{/if}
						</div>
					{/each}
				{/if}
				{#if agent === 'codex' && codexResets}
					{#if popover}
						<CodexResets credits={codexResets} {now} />
					{:else}
						<div class="resets">
							{codexResets.availableCount} {codexResets.availableCount === 1 ? 'reset' : 'resets'}{#if codexResets.availableCount > 0 && codexResets.expiresAt !== undefined}{' · '}<time datetime={new Date(codexResets.expiresAt).toISOString()}>{expiry(codexResets, now)}</time>{/if}
						</div>
					{/if}
				{/if}
			</div>
		{/each}
	</div>
{/snippet}

{#snippet tooltipDetail()}
	{@render detail(false)}
{/snippet}

<section aria-label="Usage windows" class:phone={form() === 'phone'} hidden={!visibleAgents.length}>
	<Popover.Root bind:open={() => open, (value) => { open = value; if (value) tip = false; }}>
		<Tooltip delayDuration={0} ignoreNonKeyboardFocus bind:open={() => tip, (value) => (tip = value && !open)} content={tooltipDetail}>
			{#snippet children(tooltip)}
				<Popover.Trigger>
					{#snippet child({ props })}
						<button {...mergeProps(tooltip, props)} class="trigger" aria-label="Usage details"></button>
					{/snippet}
				</Popover.Trigger>
			{/snippet}
		</Tooltip>
		<Popover.Portal>
			<Popover.Content side={form() === 'phone' ? 'bottom' : 'top'} align="start" sideOffset={4} collisionPadding={8} onOpenAutoFocus={(event) => { event.preventDefault(); popover?.focus(); }}>
				{#snippet child({ wrapperProps, props })}
					<div {...wrapperProps}>
						<div {...props} bind:this={popover} tabindex="-1" class="popover" role="dialog" aria-label="Usage details">{@render detail(true)}</div>
					</div>
				{/snippet}
			</Popover.Content>
		</Popover.Portal>
	</Popover.Root>
	<div class="row" class:crowded>
		{#each visibleAgents as { agent, label, Logo } (agent)}
			<div class="agent {agent}" role="group" aria-label={label}>
				<span class="brand" aria-hidden="true"><Logo /></span>
				{#if agent !== 'copilot'}
					{@const meters = measured(agent)}
					{@const hottest = meters.reduce((top, meter) => (meter.percent > top.percent ? meter : top), meters[0])}
					{#each meters as meter (meter.window)}
						<span class="meter" class:high={meter.percent >= high} class:secondary={meter !== hottest} role="meter" aria-label="{label} · {meter.name}" aria-valuemin={0} aria-valuemax={100} aria-valuenow={meter.percent} aria-valuetext={`${meter.percent}% used. ${details(meter.reading)}`}>
							<span class="ring">
								<svg viewBox="0 0 40 40" aria-hidden="true">
									<circle class="track" cx="20" cy="20" r="18.3" />
									<circle class="arc" cx="20" cy="20" r="18.3" pathLength="100" stroke-dasharray="{Math.min(meter.percent, 100)} 100" transform="rotate(-90 20 20)" />
								</svg>
								<span class="number">{meter.percent}</span>
							</span>
							<span class="caption">
								<span>{meter.short}</span>
								{#if meter.reading.resetsAt !== undefined}
									<span class="remaining">{remaining(meter.reading.resetsAt)}</span>
								{/if}
							</span>
						</span>
					{/each}
				{/if}
				{#if agent === 'codex' && resets > 0}
					<span class="count"><ResetIcon aria-hidden="true" />{resets}<span class="assistive">{` ${resets === 1 ? 'reset' : 'resets'}`}</span></span>
				{/if}
			</div>
		{/each}
	</div>
</section>

<style>
	section {
		position: relative;
		flex: none;
		box-sizing: border-box;
		margin-top: auto;
		border-top: var(--border-width) solid var(--border);
		font-size: var(--font-size-sm);
		container-type: inline-size;
	}

	section[hidden] {
		display: none;
	}

	.phone {
		margin: 0;
		border-top: none;
		border-bottom: var(--border-width) solid var(--border);
	}

	.trigger {
		position: absolute;
		inset: 0;
		padding: 0;
		border: none;
		background: transparent;
		cursor: pointer;
	}

	.trigger:hover,
	.trigger[data-state='open'] {
		background: var(--hover);
	}

	.trigger:focus-visible {
		outline: 2px solid var(--accent);
		outline-offset: -2px;
	}

	.row {
		--ring: 30px;
		--number: 11px;
		--label: 10px;
		--logo: 16px;
		position: relative;
		display: flex;
		gap: var(--space-3);
		padding: 4.5px var(--space-4);
		overflow: hidden;
		color: var(--muted-fg);
		white-space: nowrap;
		pointer-events: none;
	}

	.phone .row {
		--ring: 36px;
		--number: 12.5px;
		--label: 10.5px;
		--logo: 18px;
		gap: var(--space-4);
		padding-block: 5.5px;
	}

	.agent {
		display: flex;
		flex: none;
		align-items: flex-start;
		gap: var(--space-1);
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

	:global([data-scheme='dark']) .codex {
		--brand: #b7c5ee;
	}

	.brand {
		display: flex;
		flex: none;
	}

	.brand :global(svg) {
		width: var(--logo, 16px);
		height: var(--logo, 16px);
		color: var(--brand);
	}

	.agent .brand {
		align-items: center;
		height: var(--ring);
	}

	.meter {
		display: flex;
		flex: none;
		flex-direction: var(--meter-flow, row);
		align-items: center;
		gap: 1px 2px;
		line-height: 1;
	}

	.meter + .meter {
		margin-left: var(--space-1);
	}

	.ring {
		display: grid;
		place-items: center;
		width: var(--ring);
		height: var(--ring);
	}

	.ring svg,
	.number {
		grid-area: 1 / 1;
	}

	.ring svg {
		width: 100%;
		height: 100%;
		fill: none;
		stroke-width: 3.4;
	}

	.track {
		stroke: color-mix(in oklab, var(--brand) 16%, var(--surface-3));
	}

	.arc {
		stroke: var(--brand);
		transition: stroke-dasharray calc(4 * var(--dur-med)) var(--ease-out);
	}

	.number {
		color: var(--color-fg);
		font-size: var(--number);
		font-weight: 500;
		font-variant-numeric: tabular-nums;
	}

	.caption {
		display: grid;
		gap: 1px;
		font-size: var(--label);
		text-align: var(--caption-align, start);
	}

	.remaining {
		color: color-mix(in oklab, var(--color-fg) 80%, var(--muted-fg));
		font-variant-numeric: tabular-nums;
	}

	.high .arc {
		stroke: var(--status-waiting);
	}

	.high .number {
		color: var(--status-waiting);
	}

	.bar {
		display: block;
		height: 5px;
		overflow: hidden;
		border-radius: 3px;
		background: color-mix(in oklab, var(--muted-fg) 25%, transparent);
	}

	.fill {
		display: block;
		height: 100%;
		background: var(--muted-fg);
		transition: width calc(4 * var(--dur-med)) var(--ease-out);
	}

	.high .fill {
		background: var(--status-waiting);
	}

	.window.high .percent {
		color: var(--status-waiting);
		font-weight: 500;
	}

	.percent,
	.count {
		font-variant-numeric: tabular-nums;
	}

	.count {
		display: flex;
		flex: none;
		align-items: center;
		gap: 3px;
		height: var(--ring);
	}

	.assistive {
		position: absolute;
		width: 1px;
		height: 1px;
		overflow: hidden;
		clip-path: inset(50%);
	}

	@container (max-width: 319px) {
		.row {
			--meter-flow: column;
			--caption-align: center;
		}
	}

	@container (max-width: 351px) {
		.phone .row {
			--meter-flow: column;
			--caption-align: center;
		}
	}

	@container (max-width: 394px) {
		.crowded {
			--meter-flow: column;
			--caption-align: center;
		}
	}

	@container (max-width: 429px) {
		.phone .crowded {
			--meter-flow: column;
			--caption-align: center;
		}
	}

	@container (max-width: 300px) {
		.copilot,
		.secondary {
			display: none;
		}

		.meter + .meter {
			margin-left: 0;
		}
	}

	@container (max-width: 176px) {
		.row,
		.phone .row {
			--ring: 28px;
			--number: 10.5px;
			--logo: 14px;
			gap: var(--space-2);
			padding-inline: var(--space-3);
		}
	}

	.popover {
		box-sizing: border-box;
		width: min(24rem, 100vw - 2 * var(--space-4));
		padding: var(--space-2) var(--space-3);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-2);
		box-shadow: var(--shadow-float);
		color: var(--color-fg);
		font-family: var(--font-ui);
		font-size: var(--font-size-sm);
		outline: none;
	}

	.detail {
		display: grid;
		gap: var(--space-2);
		min-width: 15rem;
		white-space: normal;
	}

	.agent-detail + .agent-detail {
		padding-top: var(--space-2);
		border-top: var(--border-width) solid var(--border);
	}

	.heading {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}

	.heading strong {
		font-weight: 500;
	}

	.age,
	.period,
	.reset,
	.unit,
	.credits,
	.cycle,
	.resets {
		color: var(--muted-fg);
	}

	.age {
		margin-left: auto;
	}

	.window {
		display: grid;
		grid-template-columns: 3.5rem minmax(3rem, 1fr) 2.75rem;
		align-items: center;
		gap: 0 var(--space-2);
		padding-top: var(--space-1);
	}

	.percent {
		text-align: right;
	}

	.reset {
		display: flex;
		grid-column: 2 / -1;
		flex-wrap: wrap;
		gap: 0 var(--space-1);
	}

	.reset time::after {
		content: ' ·';
	}

	time,
	.countdown {
		white-space: nowrap;
	}

	.billing {
		display: grid;
		gap: var(--space-1);
		padding-top: var(--space-1);
	}

	.amount {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 0 var(--space-1);
		font-size: var(--font-size-md);
		font-variant-numeric: tabular-nums;
	}

	.resets {
		padding-top: var(--space-1);
	}

	.popover :global(.resets) {
		margin-top: var(--space-2);
	}
</style>
