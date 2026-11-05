<script lang="ts">
	import type { Device, Found, ServerMessage } from '#lib/device.svelte.ts';
	import type { Cursor } from '#lib/files.svelte.ts';
	import { split } from '#lib/git.ts';
	import Badge from '#lib/ui/Badge.svelte';
	import Button from '#lib/ui/Button.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import TextField from '#lib/ui/TextField.svelte';
	import Toggle from '#lib/ui/Toggle.svelte';
	import { onMount } from 'svelte';
	import Eye from '~icons/ph/eye';
	import EyeSlash from '~icons/ph/eye-slash';
	import MagnifyingGlass from '~icons/ph/magnifying-glass';
	import SlidersHorizontal from '~icons/ph/sliders-horizontal';
	import X from '~icons/ph/x';

	type Props = { projectId: string; device: Device; query: string; includeIgnored: boolean; onignored(): void; onopen(path: string, cursor: Cursor): void };
	type Result = { file: string; matches: Found[] };

	let { projectId, device, query = $bindable(), includeIgnored, onignored, onopen }: Props = $props();

	let include = $state('');
	let exclude = $state('');
	let filtering = $state(false);
	let toggles = $state({ caseSensitive: false, wholeWord: false, regex: false });
	const PAGE = 200;
	let results = $state.raw<Result[]>([]);
	let shown = $state(PAGE);
	let done = $state<{ truncated: boolean; error?: string }>();
	let input = $state<HTMLInputElement>();
	let current: string | undefined;
	let timer: ReturnType<typeof setTimeout> | undefined;
	let pending: Result[] = [];
	let frame: number | undefined;

	const total = $derived(results.reduce((sum, { matches }) => sum + matches.length, 0));
	const summary = $derived.by(() => {
		if (!total) return done ? 'No results' : 'Searching…';
		const counted = `${plural(total, 'result')} in ${plural(results.length, 'file')}`;
		if (!done) return `${counted}…`;
		return done.truncated ? `${counted} (limit reached)` : counted;
	});

	onMount(() => {
		const unlisten = device.listen({ message });
		return () => {
			clearTimeout(timer);
			if (frame !== undefined) cancelAnimationFrame(frame);
			unlisten();
		};
	});

	function message(message: ServerMessage) {
		switch (message.type) {
			case 'hello':
				if (query) run();
				break;
			case 'search.started':
				if (message.searchId !== current) break;
				reset();
				break;
			case 'search.results':
				if (message.searchId !== current) break;
				pending.push({ file: message.file, matches: message.matches });
				frame ??= requestAnimationFrame(flush);
				break;
			case 'search.done':
				if (message.searchId !== current) break;
				flush();
				done = message;
				break;
		}
	}

	function flush() {
		if (frame !== undefined) cancelAnimationFrame(frame);
		frame = undefined;
		if (!pending.length) return;
		results = results.concat(pending);
		pending = [];
	}

	function reset() {
		if (frame !== undefined) cancelAnimationFrame(frame);
		frame = undefined;
		pending = [];
		results = [];
		shown = PAGE;
		done = undefined;
	}

	function run() {
		clearTimeout(timer);
		reset();
		current = undefined;
		if (!query) return;
		current = crypto.randomUUID();
		device.send({ type: 'search.start', searchId: current, projectId, query, include: globs(include), exclude: globs(exclude), includeIgnored, ...toggles });
	}

	function flip(toggle: keyof typeof toggles) {
		toggles[toggle] = !toggles[toggle];
		run();
	}

	function flipIgnored() {
		onignored();
		run();
	}

	function clear() {
		query = '';
		run();
		input?.focus();
	}

	function schedule() {
		clearTimeout(timer);
		timer = setTimeout(run, 250);
	}

	function globs(text: string) {
		return text
			.split(',')
			.map((glob) => glob.trim())
			.filter(Boolean);
	}

	function plural(count: number, noun: string) {
		return `${count.toLocaleString()} ${noun}${count === 1 ? '' : 's'}`;
	}
</script>

{#snippet eye()}
	<IconButton type="button" label={includeIgnored ? 'Hide ignored files' : 'Show ignored files'} aria-pressed={includeIgnored} onclick={flipIgnored}>
		{#if includeIgnored}<Eye />{:else}<EyeSlash />{/if}
	</IconButton>
{/snippet}

<form
	onsubmit={(event) => {
		event.preventDefault();
		run();
	}}
>
	<div class="query">
		<div class="box">
			<MagnifyingGlass aria-hidden="true" />
			<TextField
				bind:ref={input}
				bind:value={query}
				mono
				oninput={schedule}
				placeholder="Search in files"
				aria-label="Search in files"
				aria-invalid={!!done?.error}
				enterkeyhint="search"
				autocomplete="off"
				spellcheck={false}
			/>
			{#if query}<IconButton type="button" label="Clear search" title="Clear search" onclick={clear}><X /></IconButton>{/if}
		</div>
		{#if !query}{@render eye()}{/if}
	</div>
	{#if query}
		<div class="options">
			<Toggle label="Match case" pressed={toggles.caseSensitive} onclick={() => flip('caseSensitive')}>Aa</Toggle>
			<Toggle label="Match whole word" pressed={toggles.wholeWord} onclick={() => flip('wholeWord')}><u>ab</u></Toggle>
			<Toggle label="Use regular expression" pressed={toggles.regex} onclick={() => flip('regex')}>.*</Toggle>
			<Toggle label="Include and exclude files" pressed={filtering} disabled={!!include || !!exclude} onclick={() => (filtering = !filtering)}><SlidersHorizontal /></Toggle>
			{@render eye()}
			{#if done?.error}
				<p class="error" role="alert">{done.error}</p>
			{:else}
				<p role="status">{summary}</p>
			{/if}
		</div>
		{#if filtering}
			<TextField
				bind:value={include}
				mono
				oninput={schedule}
				placeholder="Files to include, e.g. *.ts, src/**"
				aria-label="Files to include"
				enterkeyhint="search"
				autocomplete="off"
				spellcheck={false}
			/>
			<TextField
				bind:value={exclude}
				mono
				oninput={schedule}
				placeholder="Files to exclude"
				aria-label="Files to exclude"
				enterkeyhint="search"
				autocomplete="off"
				spellcheck={false}
			/>
		{/if}
	{/if}
</form>
<ul class="results" hidden={!query}>
	{#each results.slice(0, shown) as { file, matches } (file)}
		{@const { name, dir } = split(file)}
		<li>
			<div class="file" title={file}>
				<span class="name">{name}</span>
				<span class="dir">{dir}</span>
				<Badge>{matches.length}</Badge>
			</div>
			<ul aria-label={file}>
				{#each matches as { line, column, preview }, index (index)}
					<li>
						<button title={`${file}:${line}:${column}`} onclick={() => onopen(file, { line, column })}>
							<span class="line">{line}</span>
							<span class="preview">{preview}</span>
						</button>
					</li>
				{/each}
			</ul>
		</li>
	{/each}
	{#if results.length > shown}
		<li class="more"><Button onclick={() => (shown += PAGE)}>Show more</Button></li>
	{/if}
</ul>

<style>
	form {
		display: flex;
		flex: none;
		flex-direction: column;
		gap: var(--space-1);
		padding: 0 var(--space-2) var(--space-1);
	}

	.query {
		display: flex;
		align-items: center;
		gap: var(--space-1);
	}

	.box {
		position: relative;
		flex: 1;
		min-width: 0;
	}

	.box > :global(svg) {
		position: absolute;
		top: 50%;
		left: var(--space-2);
		color: var(--muted-fg);
		pointer-events: none;
		translate: 0 -50%;
	}

	.box :global(input) {
		padding-inline: calc(var(--space-2) + 1.35em) var(--target-min);
	}

	.box :global(button) {
		position: absolute;
		inset: 0 0 0 auto;
		font-size: var(--font-size-md);
	}

	.options {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0 var(--space-1);
	}

	p {
		flex: 1 1 auto;
		margin: 0;
		padding-right: var(--space-2);
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		text-align: right;
		white-space: nowrap;
	}

	.error {
		color: var(--ansi-1);
		font-family: var(--font-mono);
		text-align: left;
		white-space: pre-wrap;
	}

	ul {
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.results {
		flex: 1;
		min-height: 0;
		overflow: auto;
		padding-bottom: var(--space-3);
	}

	.more {
		padding: var(--space-2) var(--space-3);
	}

	.file {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-height: var(--target-min);
		padding: 0 var(--space-3);
		font-family: var(--font-mono);
		white-space: nowrap;
	}

	.name {
		font-size: var(--font-size-code);
	}

	.file .name {
		flex: none;
		max-width: 60%;
		overflow: hidden;
		font-weight: 600;
		text-overflow: ellipsis;
	}

	.file .dir {
		flex: 1;
		overflow: hidden;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		text-overflow: ellipsis;
	}

	.results button {
		display: flex;
		align-items: baseline;
		gap: var(--space-2);
		width: 100%;
		min-height: var(--target-min);
		padding: 0 var(--space-3) 0 var(--space-6);
		border: none;
		background: none;
		color: var(--color-fg);
		font: inherit;
		text-align: left;
		white-space: nowrap;
		cursor: pointer;
	}

	.results button:hover {
		background: var(--hover);
	}

	.results button:focus-visible {
		outline: var(--border-width) solid var(--accent);
		outline-offset: calc(-1 * var(--border-width));
	}

	.line {
		flex: none;
		min-width: 3ch;
		color: var(--muted-fg);
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		font-variant-numeric: tabular-nums;
		text-align: right;
	}

	.preview {
		overflow: hidden;
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		text-overflow: ellipsis;
		white-space: pre;
	}
</style>
