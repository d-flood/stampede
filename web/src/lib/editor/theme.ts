import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
import { EditorView } from '@codemirror/view';
import { tags } from '@lezer/highlight';

const chrome = EditorView.theme({
	'&': {
		height: '100%',
		backgroundColor: 'var(--color-bg)',
		color: 'var(--color-fg)',
		fontSize: 'var(--font-size-md)'
	},
	'&.cm-focused': { outline: 'none' },
	'.cm-scroller': { fontFamily: 'var(--font-mono)', lineHeight: '1.5' },
	'.cm-content': { caretColor: 'var(--color-cursor)' },
	'.cm-cursor, .cm-dropCursor': { borderLeftColor: 'var(--color-cursor)' },
	'.cm-fat-cursor': { background: 'var(--color-cursor)', color: 'var(--color-bg)' },
	'&:not(.cm-focused) .cm-fat-cursor': { background: 'none', outline: 'var(--border-width) solid var(--color-cursor)' },
	'.cm-selectionBackground, &.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-content ::selection':
		{ backgroundColor: 'var(--color-selection-bg)' },
	'.cm-activeLine': { backgroundColor: 'color-mix(in oklab, var(--hover) 50%, transparent)' },
	'.cm-gutters': { backgroundColor: 'var(--color-bg)', color: 'var(--muted-fg)', border: 'none' },
	'.cm-activeLineGutter': { backgroundColor: 'transparent', color: 'var(--color-fg)' },
	'.cm-foldPlaceholder': { backgroundColor: 'var(--surface-2)', border: 'none', color: 'var(--muted-fg)' },
	'.cm-matchingBracket, &.cm-focused .cm-matchingBracket': {
		backgroundColor: 'var(--surface-3)',
		outline: 'var(--border-width) solid var(--border)'
	},
	'.cm-nonmatchingBracket, &.cm-focused .cm-nonmatchingBracket': { color: 'var(--ansi-1)' },
	'.cm-searchMatch': { backgroundColor: 'color-mix(in oklab, var(--status-waiting) 30%, transparent)' },
	'.cm-searchMatch.cm-searchMatch-selected': {
		backgroundColor: 'color-mix(in oklab, var(--status-waiting) 55%, transparent)'
	},
	'.cm-selectionMatch': { backgroundColor: 'var(--surface-3)' },
	'.cm-panels': { backgroundColor: 'var(--surface-1)', color: 'var(--color-fg)' },
	'.cm-panels-top': { borderBottom: 'var(--border-width) solid var(--border)' },
	'.cm-panels-bottom': { borderTop: 'var(--border-width) solid var(--border)' },
	'.cm-textfield, .cm-button': { font: 'inherit', color: 'inherit', borderRadius: 'var(--radius-sm)' },
	'.cm-tooltip': {
		backgroundColor: 'var(--surface-2)',
		color: 'var(--color-fg)',
		border: 'var(--border-width) solid var(--border)',
		borderRadius: 'var(--radius-md)',
		boxShadow: 'var(--shadow-float)'
	},
	'.cm-tooltip-autocomplete > ul > li[aria-selected]': { backgroundColor: 'var(--hover)', color: 'var(--color-fg)' }
});

const highlight = HighlightStyle.define([
	{ tag: [tags.keyword, tags.controlKeyword, tags.moduleKeyword, tags.operatorKeyword], color: 'var(--ansi-5)' },
	{ tag: [tags.string, tags.special(tags.string), tags.character], color: 'var(--ansi-2)' },
	{ tag: [tags.regexp, tags.escape], color: 'var(--ansi-6)' },
	{ tag: [tags.number, tags.bool, tags.null, tags.atom], color: 'var(--ansi-3)' },
	{ tag: [tags.comment, tags.meta], color: 'var(--muted-fg)', fontStyle: 'italic' },
	{ tag: [tags.function(tags.variableName), tags.function(tags.propertyName)], color: 'var(--ansi-4)' },
	{ tag: [tags.typeName, tags.className, tags.namespace], color: 'var(--ansi-6)' },
	{ tag: [tags.definition(tags.variableName), tags.labelName], color: 'var(--color-fg)' },
	{ tag: [tags.propertyName, tags.attributeName], color: 'var(--ansi-3)' },
	{ tag: [tags.tagName, tags.self, tags.special(tags.variableName)], color: 'var(--ansi-1)' },
	{ tag: [tags.operator, tags.punctuation], color: 'var(--muted-fg)' },
	{ tag: tags.heading, color: 'var(--ansi-4)', fontWeight: 'bold' },
	{ tag: tags.strong, fontWeight: 'bold' },
	{ tag: tags.emphasis, fontStyle: 'italic' },
	{ tag: tags.strikethrough, textDecoration: 'line-through' },
	{ tag: [tags.link, tags.url], color: 'var(--ansi-4)', textDecoration: 'underline' },
	{ tag: tags.invalid, color: 'var(--ansi-1)' }
]);

export const theme = [chrome, syntaxHighlighting(highlight)];

const removed = (percent: number) => `color-mix(in oklab, var(--git-deleted) ${percent}%, transparent)`;
const added = (percent: number) => `color-mix(in oklab, var(--git-added) ${percent}%, transparent)`;

export const diffTheme = EditorView.theme({
	'&.cm-merge-a .cm-changedLine, & .cm-deletedChunk': { backgroundColor: removed(12) },
	'&.cm-merge-b .cm-changedLine, & .cm-inlineChangedLine': { backgroundColor: added(12) },
	'&.cm-merge-a .cm-changedText, & .cm-deletedChunk .cm-deletedText, &.cm-merge-b .cm-deletedText': {
		background: removed(30)
	},
	'&.cm-merge-b .cm-changedText': { background: added(30) },
	'&.cm-merge-a .cm-changedLineGutter, & .cm-deletedLineGutter': { background: 'var(--git-deleted)' },
	'&.cm-merge-b .cm-changedLineGutter': { background: 'var(--git-added)' },
	'& .cm-inlineChangedLineGutter': { background: 'var(--git-modified)' },
	'& .cm-collapsedLines': { background: 'var(--surface-1)', color: 'var(--muted-fg)', fontFamily: 'var(--font-ui)' },
	'.cm-deletedChunk .cm-chunkButtons': { insetInlineEnd: 'var(--space-2)', zIndex: 1 },
	'.cm-deletedChunk .cm-chunkButtons button': {
		minHeight: 'var(--target-min)',
		padding: '0 var(--space-3)',
		border: 'var(--border-width) solid var(--border)',
		borderRadius: 'var(--radius-md)',
		background: 'var(--surface-2)',
		color: 'var(--color-fg)',
		font: 'inherit',
		fontFamily: 'var(--font-ui)',
		fontSize: 'var(--font-size-sm)'
	},
	'.cm-deletedChunk .cm-chunkButtons button:hover': { background: 'var(--hover)' }
});
