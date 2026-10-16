<script lang="ts">
	import type { Project } from '#lib/device.ts';
	import { monogram } from '#lib/projects.ts';
	import Button from '#lib/ui/Button.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import type { Snippet } from 'svelte';
	import FolderOpen from '~icons/ph/folder-open';

	type Props = {
		projects: Project[];
		labels: Map<string, string>;
		focused?: string;
		onfocus(id: string): void;
		onclose(id: string): void;
		onpick(): void;
		badge?: Snippet<[Project]>;
	};

	let { projects, labels, focused, onfocus, onclose, onpick, badge }: Props = $props();
</script>

<nav aria-label="Projects">
	<ol>
		{#each projects as project (project.id)}
			{@const label = labels.get(project.id) ?? project.path}
			<li>
				<Menu context items={[{ label: 'Close', onselect: () => onclose(project.id) }]}>
					{#snippet trigger(props)}
						<button
							{...props}
							tabindex={0}
							class="project"
							class:focused={project.id === focused}
							aria-label={label}
							aria-current={project.id === focused}
							onclick={(event) => {
								// A long-press opens the menu on a timer, and its release still clicks.
								if (event.currentTarget.dataset.state !== 'open') onfocus(project.id);
							}}
						>
							<span class="monogram">{monogram(project.path)}</span>
							<span class="text">
								<strong>{label}</strong>
								<span class="path">{project.path}</span>
							</span>
							{@render badge?.(project)}
						</button>
					{/snippet}
				</Menu>
			</li>
		{/each}
	</ol>
	<Button onclick={onpick}><FolderOpen /> Open Project</Button>
</nav>

<style>
	nav {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: var(--space-2);
		min-height: 0;
		padding: var(--space-2);
		overflow-y: auto;
		user-select: none;
	}

	ol {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.project {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		box-sizing: border-box;
		width: 100%;
		padding: var(--space-2);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-1);
		color: var(--color-fg);
		font: inherit;
		text-align: start;
		cursor: pointer;
	}

	.project.focused {
		border-color: var(--color-fg);
	}

	.monogram {
		display: grid;
		flex: none;
		place-items: center;
		width: var(--target-min);
		height: var(--target-min);
		border-radius: var(--radius-md);
		background: var(--surface-3);
		font-weight: 600;
	}

	.text {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: var(--space-1);
		min-width: 0;
	}

	.path {
		overflow: hidden;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
