<script lang="ts">
	import Mockup from '../Mockup.svelte';
	import Tab from './Tab.svelte';
</script>

<div class="direction">
	<Mockup {Tab} />
</div>

<style>
	.direction {
		--radius-sm: 0;
		--radius-md: 0;
		--radius-lg: 0;
		--pane-radius: 0;
		--space-1: 5px;
		--space-2: 10px;
		--space-3: 14px;
		--space-4: 20px;
		--space-6: 28px;
		--pane-gap: 0px;
		--font-size-sm: 12px;
		--font-size-md: 15px;
		--font-size-lg: 19px;
		--border-width: 2px;
		--shadow-float: 6px 6px 0 var(--border);
		--target-min: 36px;

		display: contents;
	}

	@media (pointer: coarse) {
		.direction {
			--target-min: 48px;
		}
	}
</style>
