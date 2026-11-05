<script lang="ts">
	let { color }: { color: string } = $props();
</script>

<span class="dot" style:background-color={color} aria-hidden="true"></span>

<style>
	.dot { display: inline-block; flex: none; width: 0.6em; height: 0.6em; border-radius: 50%; }
</style>
