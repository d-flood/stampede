<script lang="ts">
	import type { Project } from '#lib/device.svelte.ts';
	import { image, list, type Entry } from '#lib/files.svelte.ts';
	import { monogram } from '#lib/projects.ts';
	import Button from '#lib/ui/Button.svelte';
	import Folder from '~icons/ph/folder';

	let { project, onclose }: { project: Project; onclose(): void } = $props();
	let website = $state('');
	let path = $state('');
	let browsing = $state(false);
	let emojis = $state(false);
	let entries = $state<Entry[]>([]);
	let selected = $state<{ blob: Blob; src: string }>();
	let error = $state<string>();
	let busy = $state(false);
	let upload: HTMLInputElement;

	async function run(action: () => Promise<void>) {
		busy = true;
		error = undefined;
		try { await action(); }
		catch (cause) { error = cause instanceof Error ? cause.message : String(cause); }
		finally { busy = false; }
	}

	async function response(result: Response) {
		if (!result.ok) throw new Error(await result.text());
		return result;
	}

	async function select(blob: Blob) {
		if (blob.size > 20 * 1024 * 1024) throw new Error('Choose an image up to 20 MiB.');
		const src = URL.createObjectURL(blob);
		try {
			const img = new Image();
			img.src = src;
			await img.decode().catch(() => { throw new Error('Unable to display this image. Choose another image.'); });
			const scale = Math.min(1, 256 / Math.max(img.naturalWidth, img.naturalHeight));
			const canvas = document.createElement('canvas');
			canvas.width = Math.max(1, Math.round(img.naturalWidth * scale));
			canvas.height = Math.max(1, Math.round(img.naturalHeight * scale));
			const context = canvas.getContext('2d');
			if (!context) throw new Error('Unable to prepare this image.');
			context.drawImage(img, 0, 0, canvas.width, canvas.height);
			const png = await new Promise<Blob>((resolve, reject) => canvas.toBlob((blob) => blob ? resolve(blob) : reject(new Error('Unable to prepare this image.')), 'image/png'));
			selected = { blob: png, src: canvas.toDataURL('image/png') };
		} finally { URL.revokeObjectURL(src); }
	}

	async function browse(directory: string) {
		await run(async () => {
			const found = await list(project.id, directory, true);
			entries = found.filter((entry) => entry.name !== '.git' && (entry.dir || image(entry.path)));
			path = directory;
			browsing = true;
			emojis = false;
		});
	}

	async function selectEmoji(unicode: string, font: string) {
		await run(async () => {
			const canvas = document.createElement('canvas');
			canvas.width = canvas.height = 256;
			const context = canvas.getContext('2d');
			if (!context) throw new Error('Unable to prepare this emoji.');
			context.font = `192px ${font}`;
			const { actualBoundingBoxLeft: left, actualBoundingBoxRight: right, actualBoundingBoxAscent: ascent, actualBoundingBoxDescent: descent } = context.measureText(unicode);
			const scale = Math.min(1, 224 / Math.max(left + right, ascent + descent));
			context.translate(128, 128);
			context.scale(scale, scale);
			context.fillText(unicode, (left - right) / 2, (ascent - descent) / 2);
			const blob = await new Promise<Blob>((resolve, reject) => canvas.toBlob((blob) => blob ? resolve(blob) : reject(new Error('Unable to prepare this emoji.')), 'image/png'));
			await select(blob);
			emojis = false;
		});
	}

	async function fetchIcon() {
		await run(async () => {
			const result = await response(await fetch('/api/project-icons/website', {
				method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ url: website })
			}));
			await select(await result.blob());
		});
	}

	async function save(initials = false) {
		await run(async () => {
			await response(await fetch(`/api/projects/${project.id}/icon`, initials ? { method: 'DELETE' } : {
				method: 'PUT', headers: { 'Content-Type': 'image/png' }, body: selected!.blob
			}));
			onclose();
		});
	}
</script>

<div class="picker" aria-busy={busy}>
	<form onsubmit={(event) => { event.preventDefault(); fetchIcon(); }}>
		<label for="project-icon-website">Website URL</label>
		<div class="website">
			<input id="project-icon-website" bind:value={website} placeholder="example.org/project/" autocomplete="url" inputmode="url" disabled={busy} />
			<Button type="submit" disabled={busy || !website.trim()}>Fetch icon</Button>
		</div>
	</form>
	<div class="sources">
		<Button disabled={busy} onclick={() => { emojis = !emojis; browsing = false; }} aria-expanded={emojis}>Choose emoji…</Button>
		<Button disabled={busy} onclick={() => browse('')}>Choose from project…</Button>
		<Button disabled={busy} onclick={() => upload.click()}>Upload image…</Button>
		<input bind:this={upload} type="file" accept="image/*,.ico,.svg" hidden onchange={() => {
			const file = upload.files?.[0];
			if (file) run(() => select(file));
			upload.value = '';
		}} />
	</div>
	{#if emojis}
		<div inert={busy}>
			{#await import('#lib/shell/EmojiPicker.svelte')}
				<p role="status">Loading emoji…</p>
			{:then { default: EmojiPicker }}
				<EmojiPicker onselect={selectEmoji} />
			{:catch}
				<p class="error" role="alert">Unable to load emoji. Close the emoji browser and try again.</p>
			{/await}
		</div>
	{/if}
	{#if browsing}
		<section aria-label="Project images">
			<div class="directory">
				<strong>{path || project.path.split('/').at(-1)}</strong>
				{#if path}<Button disabled={busy} onclick={() => browse(path.split('/').slice(0, -1).join('/'))}>Up</Button>{/if}
			</div>
			<div class="files">
				{#each entries as entry (entry.path)}
					<button class="file" disabled={busy} title={entry.path} onclick={() => {
						if (entry.dir) browse(entry.path);
						else run(async () => {
							const result = await response(await fetch(`/api/projects/${project.id}/image?${new URLSearchParams({ path: entry.path })}`));
							await select(await result.blob());
						});
					}}>
						{#if entry.dir}<Folder />{:else}<img src="/api/projects/{project.id}/image?{new URLSearchParams({ path: entry.path })}" alt="" loading="lazy" />{/if}
						<span>{entry.name}</span>
					</button>
				{/each}
				{#if !entries.length}<p>No images or folders here.</p>{/if}
			</div>
		</section>
	{/if}
	{#if busy}<p role="status">Working…</p>{/if}
	{#if error}<p class="error" role="alert">{error}</p>{/if}
	{#if selected}
		<div class="preview"><img src={selected.src} alt="Selected project icon" /><span>Preview</span></div>
	{/if}
	<div class="actions">
		<Button disabled={busy} onclick={() => save(true)}>Use initials ({monogram(project.path)})</Button>
		<Button disabled={busy} onclick={onclose}>Cancel</Button>
		<Button variant="primary" disabled={busy || !selected} onclick={() => save()}>Save</Button>
	</div>
</div>

<style>
	.picker, form { display: flex; flex-direction: column; gap: var(--space-3); }
	form { gap: var(--space-1); }
	.website, .sources, .directory, .preview, .actions { display: flex; align-items: center; gap: var(--space-2); }
	.sources, .actions { flex-wrap: wrap; }
	input:not([hidden]) {
		box-sizing: border-box;
		flex: 1;
		min-width: 0;
		min-height: var(--target-min);
		padding: 0 var(--space-2);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--color-bg);
		color: var(--color-fg);
		font: inherit;
	}
	input:focus-visible { outline: var(--border-width) solid var(--status-working); }
	.directory { justify-content: space-between; min-height: var(--target-min); }
	strong { min-width: 0; overflow-wrap: anywhere; }
	.files {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(5rem, 1fr));
		gap: var(--space-2);
		max-height: 35dvh;
		overflow-y: auto;
		padding: var(--space-1);
	}
	.file {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-1);
		min-width: 0;
		padding: var(--space-2);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-2);
		color: var(--color-fg);
		font: inherit;
		cursor: pointer;
	}
	.file:hover { background: var(--hover); }
	.file:focus-visible { outline: var(--border-width) solid var(--status-working); }
	.file span { width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: var(--font-size-sm); }
	.file img, .file :global(svg) { width: 3rem; height: 3rem; object-fit: contain; }
	p { margin: 0; color: var(--muted-fg); }
	.files p { grid-column: 1 / -1; }
	.error { color: var(--ansi-1); overflow-wrap: anywhere; }
	.preview img { width: 4rem; height: 4rem; object-fit: contain; border-radius: var(--radius-md); background: var(--surface-3); }
	.actions { justify-content: flex-end; margin-top: var(--space-2); }
</style>
