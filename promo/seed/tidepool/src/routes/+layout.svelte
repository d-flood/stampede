<script lang="ts">
	import '../app.css';

	let { children } = $props();
</script>

<svelte:head>
	<title>Tidepool</title>
</svelte:head>

{@render children()}
