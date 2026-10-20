<script lang="ts">
	import type { Todo } from '#lib/device.svelte.ts';

	let { todos }: { todos: Todo[] } = $props();
	const done = $derived(todos.filter((todo) => todo.status === 'done').length);
	const current = $derived(todos.find((todo) => todo.status === 'active')?.text);
</script>

{#if todos.length}
	<section aria-label="Todos">
		<details>
			<summary><strong>Todos {done} of {todos.length}</strong>{#if current}<span>{' · '}{current}</span>{/if}</summary>
			<ul>
				{#each todos as todo, index (index)}
					<li class:active={todo.status === 'active'} class:done={todo.status === 'done'}>
						<span aria-label={todo.status}>{todo.status === 'done' ? '✓' : todo.status === 'active' ? '◉' : '○'}</span>
						{todo.text}
					</li>
				{/each}
			</ul>
		</details>
	</section>
{/if}

<style>
	section {
		flex: none;
		padding: var(--space-2) var(--space-4);
		background: var(--surface-1);
		border-bottom: var(--border-width) solid var(--border);
	}
	details {
		max-width: 760px;
		margin: 0 auto;
		font-size: var(--font-size-sm);
	}
	summary {
		display: flex;
		align-items: center;
		cursor: pointer;
		min-height: var(--target-min);
	}
	summary strong {
		flex-shrink: 0;
	}
	summary span {
		color: var(--muted-fg);
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	ul {
		margin: 0;
		padding: 0;
		list-style: none;
		max-height: 25vh;
		overflow-y: auto;
	}
	li {
		display: flex;
		gap: var(--space-2);
		padding: var(--space-1) 0;
	}
	.active {
		color: var(--status-working);
	}
	.done {
		color: var(--muted-fg);
		text-decoration: line-through;
	}
</style>
