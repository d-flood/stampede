<script lang="ts">
	import { spinners } from '#lib/spinners.svelte.ts';
	import Claw from '#lib/ui/spinners/Claw.svelte';
	import HandPalmFill from '~icons/ph/hand-palm-fill';
</script>

<span>{#if spinners.style === 'standard'}<HandPalmFill />{:else}<Claw />{/if}</span>

<style>
	span { display: inline-flex; flex: none; --pulse: 0; }
</style>
