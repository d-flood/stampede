<script lang="ts">
	import type { Note } from '$lib/notes';

	let { notes, selected = $bindable() }: { notes: Note[]; selected?: string } = $props();
	const formatter = new Intl.DateTimeFormat(undefined, { month: 'short', day: 'numeric' });
</script>

<ul>
	{#each notes as note (note.id)}
		<li>
			<button class:active={note.id === selected} onclick={() => (selected = note.id)}>
				<strong>{note.title}</strong>
				<span>{note.body.slice(0, 80)}</span>
				<time>{formatter.format(note.updated)}</time>
			</button>
		</li>
	{/each}
</ul>

<style>
	ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}
	button {
		display: grid;
		width: 100%;
		padding: 0.75rem 1rem;
		text-align: start;
		border: none;
		background: none;
	}
	.active {
		background: var(--surface-raised);
	}
	span {
		color: var(--muted);
		overflow: hidden;
		white-space: nowrap;
		text-overflow: ellipsis;
	}
	time {
		font-size: 0.8rem;
		color: var(--muted);
	}
</style>
