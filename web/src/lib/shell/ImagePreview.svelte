<script lang="ts">
	import type { Device } from '#lib/device.svelte.ts';
	import { RequestError, type OpenFile } from '#lib/files.svelte.ts';
	import { painted } from '#lib/paint.svelte.ts';
	import { onMount } from 'svelte';

	type Props = { file: OpenFile; device: Device; active: boolean; content?: string };
	let { file, device, active, content }: Props = $props();
	let revision = $state(0);
	let src = $state<string>();
	let note = $state<string>();
	let loaded = $state(false);

	onMount(() => device.listen({
		message: (message) => {
			if (content !== undefined) return;
			if (message.type === 'hello' || (message.type === 'fs.changed' && message.projectId === file.projectId
				&& message.paths.some((path) => file.path === path || file.path.startsWith(`${path}/`)))) revision++;
		}
	}));

	$effect(() => {
		const text = content;
		const url = `/api/projects/${file.projectId}/image?${new URLSearchParams({ path: file.path })}`;
		revision;
		const controller = new AbortController();
		let objectUrl: string | undefined;
		let disposed = false;
		src = undefined;
		note = undefined;
		loaded = false;
		async function load() {
			try {
				let blob: Blob;
				if (text !== undefined) blob = new Blob([text], { type: 'image/svg+xml' });
				else {
					const response = await fetch(url, { signal: controller.signal });
					if (!response.ok) throw new RequestError(response.status, await response.text());
					blob = await response.blob();
				}
				if (disposed) return;
				if (text === undefined) file.deleted = false;
				src = objectUrl = URL.createObjectURL(blob);
			} catch (error) {
				if (disposed) return;
				if (error instanceof RequestError && error.status === 404) file.deleted = true;
				note = error instanceof Error ? error.message : 'Unable to load image';
			}
		}
		load();
		return () => {
			disposed = true;
			controller.abort();
			if (objectUrl) URL.revokeObjectURL(objectUrl);
		};
	});

	$effect(() => {
		if (active && (loaded || note)) painted(file.id);
	});
</script>

<div class="preview" aria-label="Image preview" aria-busy={!loaded && !note}>
	{#if note}
		<p role="alert">{note}</p>
	{:else if src}
		<img {src} alt={file.name} onload={() => loaded = true} onerror={() => note = 'Unable to display image'} />
	{:else}
		<p role="status">Loading image…</p>
	{/if}
</div>

<style>
	.preview {
		display: flex;
		box-sizing: border-box;
		align-items: center;
		justify-content: center;
		height: 100%;
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
