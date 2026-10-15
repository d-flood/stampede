<script lang="ts">
	import Mockup from '../Mockup.svelte';
	import Tab from './Tab.svelte';
</script>

<div class="direction">
	<Mockup {Tab} />
</div>

<style>
	.direction {
		--radius-sm: 2px;
		--radius-md: 4px;
		--radius-lg: 6px;
		--pane-radius: 0;
		--space-1: 3px;
		--space-2: 6px;
		--space-3: 9px;
		--space-4: 12px;
		--space-6: 18px;
		--pane-gap: 0px;
		--font-size-sm: 11px;
		--font-size-md: 13px;
		--font-size-lg: 15px;
		--border-width: 1px;
		--shadow-float: 0 1px 2px color-mix(in oklab, var(--ansi-0) 30%, transparent),
			0 8px 20px -6px color-mix(in oklab, var(--ansi-0) 45%, transparent);
		--target-min: 26px;

		display: contents;
	}

	@media (pointer: coarse) {
		.direction {
			--target-min: 48px;
		}
	}
</style>
