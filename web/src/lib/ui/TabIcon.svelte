<script lang="ts" module>
	import type { Agent } from '#lib/config.svelte.ts';
	import { logos } from '#lib/chat/sessions.ts';
	import Article from '~icons/ph/article';
	import Browsers from '~icons/ph/browsers';
	import Globe from '~icons/ph/globe';
	import GitDiff from '~icons/ph/git-diff';
	import TerminalWindow from '~icons/ph/terminal-window';

	export type Variant = 'agent' | 'terminal' | 'editor' | 'diff' | 'browser' | 'canvas' | 'epic';

	const icons = { terminal: TerminalWindow, diff: GitDiff, browser: Globe, canvas: Browsers, epic: Article };
</script>

<script lang="ts">
	import FileIcon from '#lib/ui/FileIcon.svelte';

	let { variant, agent = 'claude-code', label }: { variant: Variant; agent?: Agent; label: string } = $props();
</script>

{#if variant === 'editor'}
	<FileIcon path={label} />
{:else}
	{@const Icon = variant === 'agent' ? logos[agent] : icons[variant]}
	<Icon />
{/if}
