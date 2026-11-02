<script lang="ts">
	import { spinners } from '#lib/spinners.svelte.ts';
	import Dots from '#lib/ui/spinners/Dots.svelte';
	import Egg from '#lib/ui/spinners/Egg.svelte';
</script>

{#if spinners.style === 'standard'}<Dots />{:else}<Egg />{/if}
