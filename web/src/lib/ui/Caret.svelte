<script lang="ts">
	import CaretRight from '~icons/ph/caret-right';
</script>

<span class="caret" aria-hidden="true"><CaretRight /></span>

<style>
	.caret {
		display: inline-flex;
		flex: none;
		width: 1em;
		color: var(--muted-fg);
		transition: transform var(--dur-fast);
	}

	/* Turns with its control, so callers don't pass the open state twice. */
	:global([aria-expanded='true']) > .caret,
	:global(details[open] > summary) > .caret {
		transform: rotate(90deg);
	}
</style>
