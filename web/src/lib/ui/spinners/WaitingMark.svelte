<script lang="ts">
	import { spinners } from '#lib/spinners.svelte.ts';
	import Claw from '#lib/ui/spinners/Claw.svelte';
	import HandPalmFill from '~icons/ph/hand-palm-fill';

	let { pulse = false }: { pulse?: boolean } = $props();
</script>

{#if spinners.style === 'standard'}<span class:pulse><HandPalmFill /></span>{:else}<Claw />{/if}

<style>
	span { display: inline-flex; flex: none; }
	.pulse { animation: fade 1.6s ease-in-out var(--pulse); }
	@keyframes fade { 50% { opacity: 0.3; } }
</style>
