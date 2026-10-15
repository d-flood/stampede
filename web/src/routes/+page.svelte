<script lang="ts">
	const version: Promise<string> = fetch('/api/version')
		.then((response) => response.json())
		.then((body) => body.version);
</script>

<h1>Oikonomos</h1>
{#await version then version}
	<p>Version {version}</p>
{/await}
