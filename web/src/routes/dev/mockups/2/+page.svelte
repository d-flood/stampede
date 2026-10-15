<script lang="ts">
	import Mockup from '../Mockup.svelte';
	import Tab from './Tab.svelte';
</script>

<div class="direction">
	<Mockup {Tab} />
</div>

<style>
	.direction {
		--radius-sm: 6px;
		--radius-md: 10px;
		--radius-lg: 16px;
		--pane-radius: var(--radius-lg);
		--space-1: 4px;
		--space-2: 8px;
		--space-3: 12px;
		--space-4: 16px;
		--space-6: 24px;
		--pane-gap: 8px;
		--font-size-sm: 12px;
		--font-size-md: 14px;
		--font-size-lg: 17px;
		--border-width: 0px;
		--shadow-float: 0 2px 6px color-mix(in oklab, var(--ansi-0) 20%, transparent),
			0 24px 56px -12px color-mix(in oklab, var(--ansi-0) 60%, transparent);
		--target-min: 32px;

		display: contents;
	}

	@media (pointer: coarse) {
		.direction {
			--target-min: 48px;
		}
	}
</style>
