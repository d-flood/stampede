<script lang="ts">
	import { spinners } from '#lib/spinners.svelte.ts';
	import Claw from '#lib/ui/spinners/Claw.svelte';
	import HandPalmFill from '~icons/ph/hand-palm-fill';
</script>

{#if spinners.style === 'standard'}<span><HandPalmFill /></span>{:else}<Claw />{/if}

<style>
	span { display: inline-flex; flex: none; animation: fade 1.6s ease-in-out var(--pulse); }
	@keyframes fade { 50% { opacity: 0.3; } }
</style>
