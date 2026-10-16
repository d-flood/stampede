<script lang="ts">
	import type { Project } from '#lib/device.svelte.ts';
	import { monogram } from '#lib/projects.ts';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import type { Snippet } from 'svelte';
	import Plus from '~icons/ph/plus';

	type Props = {
		projects: Project[];
		labels: Map<string, string>;
		focused?: string;
		onfocus(id: string): void;
		onreorder(order: string[]): void;
		onclose(id: string): void;
		onpick(): void;
		badge?: Snippet<[Project]>;
		attention?: Snippet;
	};

	let { projects, labels, focused, onfocus, onreorder, onclose, onpick, badge, attention }: Props = $props();

	let list: HTMLOListElement;
	let drag = $state<{ id: string; y: number; order: string[]; moved: boolean }>();
	let dragged = false;

	const shown = $derived(
		drag ? drag.order.flatMap((id) => projects.filter((project) => project.id === id)) : projects
	);

	function pointerdown(event: PointerEvent) {
		const id = (event.target as HTMLElement).closest<HTMLElement>('[data-project]')?.dataset.project;
		if (!id || event.button !== 0) return;
		drag = { id, y: event.clientY, order: projects.map((project) => project.id), moved: false };
		dragged = false;
		window.addEventListener('pointermove', pointermove);
		window.addEventListener('pointerup', end);
		window.addEventListener('pointercancel', end);
	}

	function pointermove(event: PointerEvent) {
		if (!drag || (!drag.moved && Math.abs(event.clientY - drag.y) < 4)) return;
		drag.moved = true;
		const id = drag.id;
		const others = drag.order.filter((other) => other !== id);
		const above = [...list.querySelectorAll<HTMLElement>('[data-project]')].filter((row) => {
			const { top, height } = row.getBoundingClientRect();
			return row.dataset.project !== id && event.clientY > top + height / 2;
		}).length;
		drag.order = [...others.slice(0, above), id, ...others.slice(above)];
	}

	function end(event: PointerEvent) {
		window.removeEventListener('pointermove', pointermove);
		window.removeEventListener('pointerup', end);
		window.removeEventListener('pointercancel', end);
		if (drag?.moved) {
			dragged = true;
			setTimeout(() => (dragged = false));
			if (event.type === 'pointerup') onreorder(drag.order);
		}
		drag = undefined;
	}
</script>

<nav aria-label="Projects">
	<ol bind:this={list} onpointerdown={pointerdown}>
		{#each shown as project (project.id)}
			{@const label = labels.get(project.id) ?? project.path}
			<li>
				<Menu context items={[{ label: 'Close', onselect: () => onclose(project.id) }]}>
					{#snippet trigger(props)}
						<button
							{...props}
							tabindex={0}
							class="project"
							class:focused={project.id === focused}
							class:dragging={drag?.moved && drag.id === project.id}
							aria-label={label}
							aria-current={project.id === focused}
							title={label}
							data-project={project.id}
							onclick={() => {
								if (!dragged) onfocus(project.id);
							}}
						>
							{monogram(project.path)}
							{#if badge}<span class="badge">{@render badge(project)}</span>{/if}
						</button>
					{/snippet}
				</Menu>
			</li>
		{/each}
	</ol>
	<IconButton label="Open Project" onclick={onpick}><Plus /></IconButton>
	<span class="fill"></span>
	{@render attention?.()}
</nav>

<style>
	nav {
		--monogram: calc(var(--target-min) + var(--space-2));

		display: flex;
		flex: none;
		flex-direction: column;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-2);
		border-right: var(--border-width) solid var(--border);
		background: var(--surface-2);
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
		position: relative;
		width: var(--monogram);
		height: var(--monogram);
		padding: 0;
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-1);
		color: var(--muted-fg);
		font: inherit;
		font-weight: 600;
		cursor: pointer;
		touch-action: none;
		transition:
			background var(--dur-fast),
			color var(--dur-fast);
	}

	.project:hover {
		background: var(--hover);
		color: var(--color-fg);
	}

	.project:focus-visible {
		outline: var(--border-width) solid var(--status-working);
		outline-offset: var(--border-width);
	}

	.project.focused {
		border-color: var(--color-fg);
		background: var(--color-bg);
		color: var(--color-fg);
	}

	.project.focused::before {
		content: '';
		position: absolute;
		top: 20%;
		bottom: 20%;
		left: calc(-1 * var(--space-2) - var(--border-width));
		width: 3px;
		border-radius: 0 3px 3px 0;
		background: var(--color-fg);
	}

	.project.dragging {
		box-shadow: var(--shadow-float);
		cursor: grabbing;
	}

	.badge {
		position: absolute;
		top: calc(-1 * var(--space-1));
		right: calc(-1 * var(--space-1));
	}

	.fill {
		flex: 1;
	}
</style>
