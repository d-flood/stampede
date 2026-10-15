<script lang="ts">
	import { MediaQuery } from 'svelte/reactivity';
	import Desktop from './Desktop.svelte';
	import Phone from './Phone.svelte';
	import type { TabComponent } from './data.ts';

	let { Tab }: { Tab: TabComponent } = $props();

	const phone = new MediaQuery('(max-width: 640px)');
</script>

{#if phone.current}
	<Phone {Tab} />
{:else}
	<Desktop {Tab} />
{/if}
