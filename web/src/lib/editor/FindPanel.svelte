<script lang="ts">
	import Button from '#lib/ui/Button.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import TextField from '#lib/ui/TextField.svelte';
	import Toggle from '#lib/ui/Toggle.svelte';
	import {
		closeSearchPanel,
		findNext,
		findPrevious,
		getSearchQuery,
		replaceAll,
		replaceNext,
		SearchQuery,
		selectMatches,
		setSearchQuery
	} from '@codemirror/search';
	import { runScopeHandlers, type EditorView } from '@codemirror/view';
	import { untrack } from 'svelte';
	import ArrowDown from '~icons/ph/arrow-down';
	import ArrowUp from '~icons/ph/arrow-up';
	import SelectionAll from '~icons/ph/selection-all';
	import X from '~icons/ph/x';

	let { view }: { view: EditorView } = $props();

	// Mod-f focuses [main-field] when the panel is already open.
	const main = { 'main-field': 'true' };
	const editable = $derived(!view.state.readOnly);
	let spec = $state(untrack(() => fields(getSearchQuery(view.state))));
	let find = $state<HTMLInputElement>();
	let replace = $state<HTMLInputElement>();

	function fields({ search, replace, caseSensitive, regexp, wholeWord }: SearchQuery) {
		return { search, replace, caseSensitive, regexp, wholeWord };
	}

	export function sync(query: SearchQuery) {
		spec = fields(query);
	}

	function commit() {
		const query = new SearchQuery(spec);
		if (!query.eq(getSearchQuery(view.state))) view.dispatch({ effects: setSearchQuery.of(query) });
	}

	function flip(toggle: 'caseSensitive' | 'regexp' | 'wholeWord') {
		spec[toggle] = !spec[toggle];
		commit();
	}

	function keydown(event: KeyboardEvent) {
		const enter = event.key === 'Enter' && !event.isComposing;
		if (runScopeHandlers(view, event, 'search-panel')) event.preventDefault();
		else if (enter && event.target === find) {
			event.preventDefault();
			(event.shiftKey ? findPrevious : findNext)(view);
		} else if (enter && event.target === replace) {
			event.preventDefault();
			replaceNext(view);
		}
	}
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions (Editor search keys work from any control in the panel.) -->
<div class="find" role="search" onkeydown={keydown}>
	<div class="bar">
		<TextField
			bind:ref={find}
			bind:value={spec.search}
			oninput={commit}
			{...main}
			mono
			placeholder="Find"
			aria-label="Find"
			autocomplete="off"
			spellcheck={false}
		/>
		<div class="tools">
			<div class="group">
				<Toggle label="Match case" pressed={spec.caseSensitive} onclick={() => flip('caseSensitive')}>Aa</Toggle>
				<Toggle label="Match whole word" pressed={spec.wholeWord} onclick={() => flip('wholeWord')}><u>ab</u></Toggle>
				<Toggle label="Use regular expression" pressed={spec.regexp} onclick={() => flip('regexp')}>.*</Toggle>
			</div>
			<div class="group">
				<IconButton label="Previous match" onclick={() => findPrevious(view)}><ArrowUp /></IconButton>
				<IconButton label="Next match" onclick={() => findNext(view)}><ArrowDown /></IconButton>
				<IconButton label="Select all matches" onclick={() => selectMatches(view)}><SelectionAll /></IconButton>
			</div>
		</div>
		<IconButton label="Close" onclick={() => closeSearchPanel(view)}><X /></IconButton>
		{#if editable}
			<TextField
				bind:ref={replace}
				bind:value={spec.replace}
				oninput={commit}
				mono
				placeholder="Replace"
				aria-label="Replace"
				autocomplete="off"
				spellcheck={false}
			/>
			<div class="edit">
				<Button onclick={() => replaceNext(view)}>Replace</Button>
				<Button onclick={() => replaceAll(view)}>Replace all</Button>
			</div>
		{/if}
	</div>
</div>

<style>
	.find {
		container-type: inline-size;
		font-family: var(--font-ui);
		font-size: var(--font-size-md);
	}

	.bar {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto auto;
		align-items: center;
		gap: var(--space-1) var(--space-2);
		padding: var(--space-2);
	}

	.tools,
	.group,
	.edit {
		display: flex;
		align-items: center;
		gap: var(--space-1);
	}

	.tools {
		justify-content: space-between;
		gap: var(--space-2);
	}

	.edit > :global(button) {
		flex: 1;
	}

	@container (max-width: 32rem) {
		.bar {
			grid-template-columns: minmax(0, 1fr) auto;
		}

		.tools,
		.edit {
			grid-column: 1 / -1;
			order: 1;
		}
	}
</style>
