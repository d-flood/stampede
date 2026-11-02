<script lang="ts">
	import { spinners } from '#lib/spinners.svelte.ts';
	import Notch from '#lib/ui/spinners/Notch.svelte';
	import Sprint from '#lib/ui/spinners/Sprint.svelte';
</script>

{#if spinners.style === 'standard'}<Notch />{:else}<Sprint />{/if}
