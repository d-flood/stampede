<script lang="ts">
	import 'emoji-picker-element';
	import type { EmojiClickEvent } from 'emoji-picker-element/shared.js';
	import dataSource from 'emoji-picker-element-data/en/emojibase/data.json?url';

	let { onselect }: { onselect(unicode: string, font: string): void } = $props();

	function choose(event: EmojiClickEvent) {
		if (event.detail.unicode) {
			const font = getComputedStyle(event.currentTarget as HTMLElement).getPropertyValue('--emoji-font-family');
			onselect(event.detail.unicode, font);
		}
	}
</script>

<emoji-picker data-source={dataSource} onemoji-click={choose}></emoji-picker>

<style>
	emoji-picker {
		width: 100%;
		height: min(22rem, 45dvh);
		--background: var(--surface-1);
		--border-color: var(--border);
		--border-radius: var(--radius-md);
		--button-active-background: var(--surface-3);
		--button-hover-background: var(--hover);
		--category-emoji-size: 1.375rem;
		--category-emoji-padding: 0.375rem;
		--category-font-color: var(--muted-fg);
		--emoji-size: 1.5rem;
		--emoji-padding: 0.625rem;
		--font-family: var(--font-ui);
		--indicator-color: var(--status-working);
		--input-border-color: var(--border);
		--input-border-radius: var(--radius-md);
		--input-font-color: var(--color-fg);
		--input-placeholder-color: var(--muted-fg);
		--outline-color: var(--status-working);
		--num-columns: 7;
	}

	@media (max-width: 380px) {
		emoji-picker { --num-columns: 6; --category-emoji-size: 1.125rem; }
	}
</style>
