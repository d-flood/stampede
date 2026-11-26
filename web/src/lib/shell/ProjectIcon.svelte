<script lang="ts">
	import type { Project } from '#lib/device.svelte.ts';
	import { hue, monogram } from '#lib/projects.ts';

	let { project }: { project: Project } = $props();
	let failed = $state<string>();
	const image = $derived(!!project.icon && failed !== project.icon);
</script>

<span class="icon squircle" class:monogram={!image} style:--hue="var(--ansi-{hue(project.path)})" aria-hidden="true">
	{#if image}
		<img class="squircle" src="/api/project-icons/{project.icon}" alt="" onerror={() => (failed = project.icon)} />
	{:else}
		{monogram(project.path, 1)}
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
		background: var(--tile-base, var(--surface-1));
		color: color-mix(in oklab, var(--hue), var(--color-fg) 20%);
		font-weight: 700;
	}

	img {
		box-sizing: border-box;
		width: 100%;
		height: 100%;
		min-width: 0;
		min-height: 0;
		padding: var(--icon-pad, var(--space-1));
		border-radius: inherit;
		object-fit: contain;
	}
</style>
