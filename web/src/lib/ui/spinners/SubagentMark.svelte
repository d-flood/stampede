<script lang="ts">
	import { spinners } from '#lib/spinners.svelte.ts';
	import Notch from '#lib/ui/spinners/Notch.svelte';
	import Trek from '#lib/ui/spinners/Trek.svelte';
</script>

{#if spinners.style === 'standard'}<Notch />{:else}<Trek />{/if}
