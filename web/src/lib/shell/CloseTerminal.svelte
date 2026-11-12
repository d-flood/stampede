<script lang="ts" module>
	import type { Terminal } from '#lib/device.svelte.ts';

	export function running(terminal: Terminal) {
		return terminal.state === 'live' && !!terminal.running?.length;
	}
</script>

<script lang="ts">
	import Stopping from '#lib/shell/Stopping.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';

	let { terminal = $bindable(), onclose }: { terminal?: Terminal; onclose: (terminal: Terminal) => void } = $props();
</script>

<Dialog
	bind:open={() => !!terminal, (open) => !open && (terminal = undefined)}
	title="Close Terminal?"
	description="Closing it stops these and everything they started."
>
	<Stopping items={(terminal?.running ?? []).map((label) => ({ kind: 'command', label }))} />
	{#snippet actions()}
		<Button onclick={() => (terminal = undefined)}>Cancel</Button>
		<Button variant="primary" onclick={() => terminal && onclose(terminal)}>Close</Button>
	{/snippet}
</Dialog>
