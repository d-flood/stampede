<script lang="ts">
	import { selectedTheme } from '#lib/config.svelte.ts';
	import { form } from '#lib/form.ts';
	import ThemeSwitcher from '#lib/shell/ThemeSwitcher.svelte';
	import { selectTextSize, textSize, typography } from '#lib/typography.svelte.ts';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Gear from '~icons/ph/gear';

	let open = $state(false);
</script>

<IconButton label="Settings" aria-haspopup="dialog" aria-expanded={open} onclick={() => (open = true)}><Gear /></IconButton>
<Dialog bind:open full={form() === 'phone'} title="Settings">
	<div class="settings">
		<section aria-labelledby="text-size-label">
			<div class="row">
				<label id="text-size-label" for="text-size">Text size</label>
				<select id="text-size" value={textSize()} onchange={(event) => selectTextSize(Number(event.currentTarget.value))} aria-describedby="text-size-help">
					{#each Array.from({ length: 11 }, (_, index) => index + 12) as size}
						<option value={size}>{size} px</option>
					{/each}
				</select>
			</div>
			<p id="text-size-help">Applies to the UI, chats, editor, and terminals. Saved in this browser.</p>
			<div class="preview" aria-label="Text preview">
				<span>The quick brown fox jumps over the lazy dog.</span>
				<code>const greeting = 'Hello, world!';</code>
				<small>Labels and supporting text</small>
			</div>
			<div class="row">
				<p>{typography.size === null ? 'Using default: 16 px on mobile, 13 px on desktop.' : 'Custom text size'}</p>
				<Button disabled={typography.size === null} onclick={() => selectTextSize(null)}>Reset</Button>
			</div>
		</section>
		<section aria-labelledby="theme-label">
			<div class="row">
				<div><h3 id="theme-label">Theme</h3><p>{selectedTheme.name ?? 'Default'}</p></div>
				<ThemeSwitcher />
			</div>
			<p>Applies on every device.</p>
		</section>
	</div>
	{#snippet actions()}
		<Button onclick={() => (open = false)}>Done</Button>
	{/snippet}
</Dialog>

<style>
	.settings {
		display: grid;
		gap: var(--space-6);
	}

	section + section {
		padding-top: var(--space-4);
		border-top: var(--border-width) solid var(--border);
	}

	.row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3);
	}

	label,
	h3 {
		margin: 0;
		font-size: var(--font-size-md);
		font-weight: 600;
	}

	p {
		margin: var(--space-2) 0;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		line-height: 1.5;
	}

	select {
		min-height: var(--target-min);
		padding: var(--space-1) var(--space-2);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--color-bg);
		color: var(--color-fg);
		font: inherit;
	}

	select:focus-visible {
		outline: var(--border-width) solid var(--status-working);
		outline-offset: var(--border-width);
	}

	.preview {
		display: grid;
		gap: var(--space-3);
		margin: var(--space-4) 0;
		padding: var(--space-4);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--color-bg);
		line-height: 1.5;
		overflow-wrap: anywhere;
	}

	code {
		font-family: var(--font-mono);
	}

	small {
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}
</style>
