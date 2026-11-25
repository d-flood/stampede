<script lang="ts">
	import { logos } from '#lib/chat/sessions.ts';
	import type { Reading, ResetCredits } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';
	import CodexResets, { expiry } from '#lib/shell/CodexResets.svelte';
	import Popover from '#lib/ui/Popover.svelte';
	import Tooltip from '#lib/ui/Tooltip.svelte';
	import { mergeProps } from 'bits-ui';
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
		{ agent: 'antigravity', label: 'Google Antigravity', Logo: logos.antigravity },
		{ agent: 'copilot', label: 'Copilot', Logo: logos.copilot }
	] as const;
	const high = 80;

	const visibleAgents = $derived(agents.filter(({ agent }) => (agent === 'codex' && codexResets !== null) || readings.some((reading) => reading.agent === agent && (agent === 'copilot' || reading.used !== undefined))));
	const resets = $derived(codexResets?.availableCount ?? 0);
	const copilot = $derived.by(() => {
		const reading = readings.find((reading) => reading.agent === 'copilot' && reading.window === 'monthly');
		const currentSessions = readings.filter((reading) => reading.agent === 'copilot' && reading.window === 'session');
		const unit = currentSessions.some((reading) => reading.consumption?.unit === 'credits') ? 'credits' : 'requests';
		const sessions = currentSessions.filter((reading) => reading.consumption?.unit === unit);
		const consumption = reading?.consumption ?? (sessions.length ? { amount: sessions.reduce((total, reading) => total + reading.consumption!.amount, 0), unit } : undefined);
		const partial = !reading?.consumption && (sessions.some((reading) => reading.partial) || sessions.length < currentSessions.length);
		return { reading, sessions, consumption, partial };
	});

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
		const own = readings.filter((reading) => reading.agent === agent);
		return [...new Set(own.map((reading) => reading.group?.name))].flatMap((group) =>
			windows.flatMap(({ window, label, name, short }) =>
				own.flatMap((reading) => {
					if (reading.group?.name !== group || reading.window !== window || reading.used === undefined) return [];
					const percent = Math.round(reading.used * 100);
					return [reading.group
						? { group, window, label, name: `${reading.group.name} ${name}`, short: `${reading.group.short} ${short}`, reading, percent }
						: { group, window, label, name, short, reading, percent }];
				})
			)
		);
	}

	function hottest(agent: string) {
		const meters = measured(agent);
		return windows.flatMap(({ window, name, short }) => {
			const shown = meters.filter((meter) => meter.window === window);
			return shown.length ? [{ ...shown.reduce((top, meter) => (meter.percent > top.percent ? meter : top)), name, short }] : [];
		});
	}
</script>

{#snippet billing()}
	{@const { reading, sessions, consumption, partial } = copilot}
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
	<div class="detail" class:popover>
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
					{@const meters = measured(agent)}
					{#each meters as { group, name, label: period, reading, percent }, index (name)}
						{#if group && group !== meters[index - 1]?.group}<span class="group">{group}</span>{/if}
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

<svelte:boundary onerror={(error) => console.error(error)}>
	<section aria-label="Usage windows" class:phone={form() === 'phone'} hidden={!visibleAgents.length}>
		<Popover
			bind:open={() => open, (value) => { open = value; if (value) tip = false; }}
			bind:ref={popover}
			label="Usage details"
			side={form() === 'phone' ? 'bottom' : 'top'}
			width="24rem"
			onopenautofocus={(event) => { event.preventDefault(); popover?.focus(); }}
		>
			{#snippet trigger(props)}
				<Tooltip delayDuration={0} ignoreNonKeyboardFocus bind:open={() => tip, (value) => (tip = value && !open)} content={tooltipDetail}>
					{#snippet children(tooltip)}
						<button {...mergeProps(props, tooltip)} class="trigger" aria-label="Usage details"></button>
					{/snippet}
				</Tooltip>
			{/snippet}
			{@render detail(true)}
		</Popover>
		<div class="row">
			{#each visibleAgents as { agent, label, Logo } (agent)}
				<div class="agent {agent}" role="group" aria-label={label}>
					<span class="brand" aria-hidden="true"><Logo /></span>
					{#if agent === 'copilot'}
						{#if copilot.consumption}
							{@const { amount, unit } = copilot.consumption}
							<span class="cost">{unit === 'credits' ? cost(amount) : `${amount.toLocaleString(undefined, { maximumFractionDigits: 2 })} ${unit}`}{#if copilot.reading?.consumption}{' '}<span class="month">this month</span>{/if}</span>
						{/if}
					{:else}
						{#each hottest(agent) as meter (meter.name)}
							<span class="meter {meter.window}" class:high={meter.percent >= high} role="meter" aria-label="{label} · {meter.name}" aria-valuemin={0} aria-valuemax={100} aria-valuenow={meter.percent} aria-valuetext={`${meter.percent}% used. ${details(meter.reading)}`}>
								<span class="period">{meter.short}</span>
								<svg class="ring" viewBox="0 0 20 20" aria-hidden="true"><circle class="track" cx="10" cy="10" r="8" /><circle class="arc" cx="10" cy="10" r="8" pathLength="100" stroke-dasharray="{Math.min(meter.percent, 100)} 100" transform="rotate(-90 10 10)" /></svg>
								<span class="number">{meter.percent}</span>
								{#if meter.reading.resetsAt !== undefined}
									<span class="remaining">{remaining(meter.reading.resetsAt)}</span>
								{/if}
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
</svelte:boundary>

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
		outline: var(--focus-ring);
		outline-offset: var(--focus-inset);
	}

	.row {
		--number: 11px;
		--label: 10px;
		--logo: 14px;
		position: relative;
		display: grid;
		grid-template-columns: auto repeat(2, auto auto auto auto minmax(0, 1fr)) auto;
		row-gap: var(--space-2);
		align-items: center;
		padding: var(--space-2) var(--space-4);
		overflow: hidden;
		color: var(--muted-fg);
		font-size: var(--label);
		line-height: 1;
		white-space: nowrap;
		pointer-events: none;
	}

	.phone .row {
		--number: 12.5px;
		--label: 10.5px;
		--logo: 16px;
		padding-block: var(--space-3);
	}

	.agent,
	.meter {
		display: grid;
		grid-template-columns: subgrid;
	}

	.agent {
		grid-column: 1 / -1;
		align-items: center;
	}

	.meter {
		align-items: baseline;
	}

	.five-hour {
		grid-column: 2 / span 4;
	}

	.weekly {
		grid-column: 7 / span 4;
	}

	.claude-code {
		--brand: #d97757;
	}

	.codex {
		--brand: #52669e;
	}

	.antigravity {
		--brand: #8e75b2;
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

	.meter .period {
		padding-inline: var(--space-3) var(--space-1);
	}

	.ring {
		align-self: center;
		width: var(--logo);
		height: var(--logo);
		margin-right: var(--space-1);
		fill: none;
		stroke-width: 3.5;
	}

	.ring .track {
		stroke: color-mix(in oklab, var(--brand) 22%, var(--surface-3));
	}

	.ring .arc {
		stroke: var(--brand);
		stroke-linecap: round;
		transition: stroke-dasharray calc(4 * var(--dur-med)) var(--ease-out);
	}

	.high .ring .arc {
		stroke: var(--status-waiting);
	}

	.number {
		justify-self: end;
		color: var(--color-fg);
		font-size: var(--number);
		font-weight: 500;
		font-variant-numeric: tabular-nums;
	}

	.remaining {
		padding-left: var(--space-2);
		color: color-mix(in oklab, var(--color-fg) 80%, var(--muted-fg));
		font-variant-numeric: tabular-nums;
	}

	.high .number {
		color: var(--status-waiting);
	}

	.cost {
		grid-column: 2 / -1;
		overflow: hidden;
		padding-left: var(--space-3);
		color: var(--color-fg);
		font-size: var(--number);
		font-variant-numeric: tabular-nums;
		text-overflow: ellipsis;
	}

	.month {
		color: var(--muted-fg);
		font-size: var(--label);
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
		grid-column: 12;
		align-items: center;
		gap: var(--space-1);
		padding-left: var(--space-3);
		font-size: var(--number);
	}

	.assistive {
		position: absolute;
		width: 1px;
		height: 1px;
		overflow: hidden;
		clip-path: inset(50%);
	}

	@container (width < 299px) {
		.remaining {
			display: none;
		}
	}

	/* Percent signs need the room that reset times give up below 299px. */
	@container (270px <= width < 299px) or (width >= 330px) {
		.number::after {
			content: '%';
			color: var(--muted-fg);
			font-weight: 400;
		}
	}

	@container (width < 330px) {
		.phone .remaining {
			display: none;
		}
	}

	@container (width < 216px) {
		.row {
			padding-inline: var(--space-3);
		}

		.row .ring {
			display: none;
		}
	}

	@container (width < 244px) {
		.phone .row {
			padding-inline: var(--space-3);
		}

		.phone .ring {
			display: none;
		}
	}

	.detail {
		display: grid;
		gap: var(--space-2);
		font-size: var(--font-size-sm);
		min-width: 15rem;
		white-space: normal;
	}

	.agent-detail {
		display: grid;
		grid-template-columns: minmax(3.5rem, max-content) minmax(3rem, 1fr) 2.75rem;
		column-gap: var(--space-2);
	}

	.agent-detail > :global(*) {
		grid-column: 1 / -1;
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
		grid-template-columns: subgrid;
		align-items: center;
		padding-top: var(--space-1);
	}

	.group {
		padding-top: var(--space-1);
	}

	.window + .group {
		padding-top: var(--space-2);
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
