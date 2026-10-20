<script lang="ts">
	import type { Project } from '#lib/device.svelte.ts';
	import { monogram } from '#lib/projects.ts';

	let { project }: { project: Project } = $props();
	let failed = $state<string>();
</script>

<span class="icon" aria-hidden="true">
	{#if project.icon && failed !== project.icon}
		<img src="/api/project-icons/{project.icon}" alt="" onerror={() => (failed = project.icon)} />
	{:else}
		{monogram(project.path)}
	{/if}
</span>

<style>
	.icon {
		display: grid;
		place-items: center;
		width: 100%;
		height: 100%;
		border-radius: inherit;
	}

	img {
		box-sizing: border-box;
		width: 100%;
		height: 100%;
		min-width: 0;
		min-height: 0;
		padding: var(--space-1);
		border-radius: inherit;
		object-fit: contain;
	}
</style>
