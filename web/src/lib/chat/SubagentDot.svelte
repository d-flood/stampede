<script lang="ts">
	let { color, pulse = false }: { color: string; pulse?: boolean } = $props();
</script>

<span class="dot" class:pulse style:background-color={color} aria-hidden="true"></span>

<style>
	.dot { display: inline-block; flex: none; width: 0.6em; height: 0.6em; border-radius: 50%; }
	.pulse { animation: fade 1.6s ease-in-out var(--pulse); }
	@keyframes fade { 50% { opacity: 0.3; } }
</style>
