<script lang="ts">
	import type { Project } from '#lib/device.svelte.ts';
	import { monogram } from '#lib/projects.ts';

	let { project }: { project: Project } = $props();
	let failed = $state<string>();
	const image = $derived(!!project.icon && failed !== project.icon);
</script>

<span class="icon squircle" class:monogram={!image} aria-hidden="true">
	{#if image}
		<img class="squircle" src="/api/project-icons/{project.icon}" alt="" onerror={() => (failed = project.icon)} />
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

	.monogram {
		background: color-mix(in oklab, var(--hue) 15%, transparent);
		color: color-mix(in oklab, var(--hue), var(--color-fg) 30%);
		font-weight: 700;
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
