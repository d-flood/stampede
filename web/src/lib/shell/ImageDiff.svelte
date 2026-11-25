<script lang="ts" module>
	/** One version of the image; `live` names what changes it without its URL changing. */
	export type ImageSide = { label: string; url?: string; live?: 'disk' | 'index' };
</script>

<script lang="ts">
	import type { Device } from '#lib/device.svelte.ts';
	import type { OpenDiff } from '#lib/files.svelte.ts';
	import { painted } from '#lib/paint.svelte.ts';
	import { onDestroy, onMount } from 'svelte';

	type Pane = { src?: string; bytes?: number; note?: string };
	type Props = { diff: OpenDiff; sides: ImageSide[]; device: Device; active: boolean };
	let { diff, sides, device, active }: Props = $props();

	let panes = $state.raw<Pane[]>();
	let sizes = $state<Record<string, { width: number; height: number } | 'broken'>>({});
	let revision = $state(0);
	let held: string[] = [];

	onMount(() => device.listen({
		message(message) {
			if (message.type === 'hello') revision++;
			else if (message.type === 'git.changed' && message.projectId === diff.projectId && sides.some((side) => side.live === 'index')) revision++;
			else if (message.type === 'fs.changed' && message.projectId === diff.projectId && sides.some((side) => side.live === 'disk')
				&& message.paths.some((path) => diff.path === path || diff.path.startsWith(`${path}/`))) revision++;
		}
	}));

	onDestroy(() => hold([]));

	$effect(() => {
		const [wanted, svg] = [sides, /\.svg$/i.test(diff.path)];
		revision;
		const controller = new AbortController();
		const made: string[] = [];
		Promise.all(wanted.map(async (side): Promise<Pane> => {
			if (!side.url) return { note: `Not in ${side.label}` };
			const response = await fetch(side.url, { signal: controller.signal });
			if (side.live === 'disk') diff.deleted = response.status === 404;
			if (response.status === 404) return { note: `Not in ${side.label}` };
			if (!response.ok) return { note: (await response.text()) || 'Unable to load image' };
			const blob = await response.blob();
			if (!blob.size) return { note: `Not in ${side.label}` };
			const src = URL.createObjectURL(svg ? new Blob([blob], { type: 'image/svg+xml' }) : blob);
			made.push(src);
			return { src, bytes: blob.size };
		})).then(
			(next) => {
				if (controller.signal.aborted) return;
				hold(made);
				panes = next;
			},
			(error: Error) => {
				if (!controller.signal.aborted) panes = wanted.map(() => ({ note: error.message }));
			}
		);
		return () => {
			controller.abort();
			if (held !== made) for (const url of made) URL.revokeObjectURL(url);
		};
	});

	$effect(() => {
		if (active && panes) painted(diff.id);
	});

	/** Keeps the shown URLs alive until their replacements are, so a reload doesn't flash. */
	function hold(urls: string[]) {
		for (const url of held) {
			URL.revokeObjectURL(url);
			delete sizes[url];
		}
		held = urls;
	}

	function measure(src: string, image: HTMLImageElement) {
		sizes[src] = { width: image.naturalWidth, height: image.naturalHeight };
	}

	function caption({ src, bytes }: Pane) {
		const size = src && sizes[src];
		const amount = bytes === undefined ? '' : bytes < 1024 ? `${bytes} B` : bytes < 1024 ** 2 ? `${(bytes / 1024).toFixed(1)} KB` : `${(bytes / 1024 ** 2).toFixed(1)} MB`;
		return [size && size !== 'broken' && `${size.width} × ${size.height}`, amount].filter(Boolean).join(' · ');
	}
</script>

<div class="images" class:stacked={diff.layout === 'unified'} aria-busy={!panes}>
	{#each sides as side, index (index)}
		{@const pane = panes?.[index]}
		<figure>
			<figcaption>
				<span class="label" title={side.label}>{side.label}</span>
				{#if pane}<span class="facts">{caption(pane)}</span>{/if}
			</figcaption>
			<div class="frame">
				{#if !pane}
					<p role="status">Loading image…</p>
				{:else if pane.src && sizes[pane.src] === 'broken'}
					<p role="alert">Unable to display image</p>
				{:else if pane.src}
					{@const src = pane.src}
					<img {src} alt="{diff.name} in {side.label}" onload={(event) => measure(src, event.currentTarget as HTMLImageElement)} onerror={() => (sizes[src] = 'broken')} />
				{:else}
					<p>{pane.note}</p>
				{/if}
			</div>
		</figure>
	{/each}
</div>

<style>
	.images {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		height: 100%;
	}

	.stacked {
		grid-template-rows: repeat(2, minmax(0, 1fr));
		grid-template-columns: minmax(0, 1fr);
	}

	figure {
		display: flex;
		flex-direction: column;
		min-width: 0;
		min-height: 0;
		margin: 0;
	}

	figure + figure {
		border-left: var(--border-width) solid var(--border);
	}

	.stacked figure + figure {
		border-top: var(--border-width) solid var(--border);
		border-left: none;
	}

	figcaption {
		display: flex;
		flex: none;
		align-items: baseline;
		gap: var(--space-2);
		padding: var(--space-1) var(--space-3);
		border-bottom: var(--border-width) solid var(--border);
		background: var(--surface-1);
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.label {
		overflow: hidden;
		color: var(--color-fg);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.facts {
		flex: none;
		margin-left: auto;
		font-variant-numeric: tabular-nums;
	}

	.frame {
		display: flex;
		flex: 1;
		box-sizing: border-box;
		align-items: center;
		justify-content: center;
		min-height: 0;
		overflow: auto;
		padding: var(--space-4);
	}

	img {
		display: block;
		max-width: 100%;
		max-height: 100%;
		object-fit: contain;
	}

	p {
		color: var(--muted-fg);
		text-align: center;
	}
</style>
