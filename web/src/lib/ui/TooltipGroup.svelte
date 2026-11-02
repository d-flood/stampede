<script module lang="ts">
	export const grouped = Symbol('tooltip group');
</script>

<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import { setContext, type Snippet } from 'svelte';

	let { children }: { children: Snippet } = $props();
	setContext(grouped, true);
</script>

<Tooltip.Provider>{@render children()}</Tooltip.Provider>
