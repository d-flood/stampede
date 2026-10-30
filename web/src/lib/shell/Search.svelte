<script lang="ts">
	import type { Device, Found, ServerMessage } from '#lib/device.svelte.ts';
	import type { Cursor } from '#lib/files.svelte.ts';
	import { form } from '#lib/form.ts';
	import { split } from '#lib/git.ts';
	import Badge from '#lib/ui/Badge.svelte';
	import Button from '#lib/ui/Button.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import TextField from '#lib/ui/TextField.svelte';
	import Toggle from '#lib/ui/Toggle.svelte';
	import { onMount } from 'svelte';
	import Eye from '~icons/ph/eye';
	import EyeSlash from '~icons/ph/eye-slash';

	type Props = { projectId: string; device: Device; hidden: boolean; onopen(path: string, cursor: Cursor): void };
	type Result = { file: string; matches: Found[] };

	let { projectId, device, hidden, onopen }: Props = $props();

	let query = $state('');
	let include = $state('');
	let exclude = $state('');
	let toggles = $state({ caseSensitive: false, wholeWord: false, regex: false, includeIgnored: false });
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

	$effect(() => {
		if (!hidden && form() === 'desktop') input?.focus();
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
		device.send({ type: 'search.start', searchId: current, projectId, query, include: globs(include), exclude: globs(exclude), ...toggles });
	}

	function flip(toggle: keyof typeof toggles) {
		toggles[toggle] = !toggles[toggle];
		run();
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

<section aria-label="Search" {hidden}>
	<form
		onsubmit={(event) => {
			event.preventDefault();
			run();
		}}
	>
		<div class="query">
			<TextField
				bind:ref={input}
				bind:value={query}
				mono
				oninput={schedule}
				placeholder="Search"
				aria-label="Search"
				aria-invalid={!!done?.error}
				enterkeyhint="search"
				autocomplete="off"
				spellcheck={false}
			/>
			<Toggle label="Match case" pressed={toggles.caseSensitive} onclick={() => flip('caseSensitive')}>Aa</Toggle>
			<Toggle label="Match whole word" pressed={toggles.wholeWord} onclick={() => flip('wholeWord')}><u>ab</u></Toggle>
			<Toggle label="Use regular expression" pressed={toggles.regex} onclick={() => flip('regex')}>.*</Toggle>
		</div>
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
	</form>
	<header>
		{#if done?.error}
			<p class="error" role="alert">{done.error}</p>
		{:else}
			<p role="status">{query ? summary : ''}</p>
		{/if}
		<IconButton
			label={toggles.includeIgnored ? 'Skip ignored files' : 'Search ignored files'}
			aria-pressed={toggles.includeIgnored}
			onclick={() => flip('includeIgnored')}
		>
			{#if toggles.includeIgnored}<Eye />{:else}<EyeSlash />{/if}
		</IconButton>
	</header>
	<ul class="results">
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
</section>

<style>
	section {
		display: flex;
		flex: 1;
		flex-direction: column;
		min-height: 0;
	}

	section[hidden] {
		display: none;
	}

	form {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		padding: 0 var(--space-2);
	}

	.query {
		display: flex;
		gap: var(--space-1);
	}

	header {
		display: flex;
		align-items: center;
		padding: 0 var(--space-1) 0 var(--space-3);
	}

	p {
		flex: 1;
		margin: 0;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.error {
		color: var(--ansi-1);
		font-family: var(--font-mono);
		white-space: pre-wrap;
	}

	ul {
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.results {
		flex: 1;
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
		outline: var(--border-width) solid var(--status-working);
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
