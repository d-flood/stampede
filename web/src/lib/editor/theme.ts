import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
import { EditorView } from '@codemirror/view';
import { tags } from '@lezer/highlight';

const chrome = EditorView.theme({
	'&': {
		height: '100%',
		backgroundColor: 'var(--color-bg)',
		color: 'var(--color-fg)',
		fontSize: 'var(--font-size-code)'
	},
	'&.cm-focused': { outline: 'none' },
	'.cm-scroller': { fontFamily: 'var(--font-mono)', lineHeight: '1.5' },
	'.cm-content': { caretColor: 'var(--color-cursor)', fontVariantLigatures: 'none' },
	'.cm-content:not(.cm-lineWrapping)': { paddingRight: 'calc(2 * var(--space-6) + var(--space-4))' },
	// Fades lines running past the pane's right edge into a right inset instead of cutting them mid-glyph.
	'.cm-scroller:not(:has(> .cm-lineWrapping))::after': {
		content: '""',
		position: 'sticky',
		right: 0,
		flex: 'none',
		alignSelf: 'stretch',
		width: 'calc(2 * var(--space-6) + var(--space-4))',
		marginLeft: 'calc(-2 * var(--space-6) - var(--space-4))',
		background: 'linear-gradient(to right, transparent, var(--color-bg) calc(100% - var(--space-4)))',
		pointerEvents: 'none'
	},
	'.cm-cursor, .cm-dropCursor': { borderLeftColor: 'var(--color-cursor)' },
	'.cm-fat-cursor': { background: 'var(--color-cursor)', color: 'var(--color-bg)' },
	'&:not(.cm-focused) .cm-fat-cursor': { background: 'none', outline: 'var(--border-width) solid var(--color-cursor)' },
	'.cm-selectionBackground, &.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-content ::selection':
		{ backgroundColor: 'var(--color-editor-selection)' },
	'.cm-activeLine': { backgroundColor: 'color-mix(in oklab, var(--hover) 50%, transparent)' },
	'.cm-gutters': { backgroundColor: 'var(--color-bg)', color: 'color-mix(in oklab, var(--muted-fg) 70%, var(--color-bg))', border: 'none' },
	'.cm-gutter:first-child .cm-gutterElement': { paddingLeft: 'var(--space-4)' },
	'.cm-activeLineGutter': { backgroundColor: 'transparent', color: 'var(--color-fg)' },
	'.cm-foldGutter .cm-gutterElement': { padding: 0 },
	'.cm-foldGutter .cm-gutterElement > *': { opacity: 0, transition: 'opacity var(--dur-fast)' },
	'.cm-gutters:hover .cm-foldGutter .cm-gutterElement > *': { opacity: 1 },
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
	'.cm-selectionMatch': { backgroundColor: 'var(--border)' },
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
	'.cm-tooltip-autocomplete > ul > li[aria-selected]': { backgroundColor: 'var(--hover)', color: 'var(--color-fg)' },
	'.cm-lintRange-error': { backgroundImage: 'none', textDecoration: 'underline wavy var(--ansi-1)' },
	'.cm-lintRange-warning': { backgroundImage: 'none', textDecoration: 'underline wavy var(--ansi-3)' },
	'.cm-lintRange-info': { backgroundImage: 'none', textDecoration: 'underline wavy var(--ansi-4)' },
	'.cm-lintRange-hint': { backgroundImage: 'none', textDecoration: 'underline dotted var(--muted-fg)' },
	'.cm-lint-unnecessary': { opacity: '0.55' },
	'.cm-lint-deprecated': { textDecoration: 'line-through' },
	'.cm-diagnostic-error': { borderLeftColor: 'var(--ansi-1)' },
	'.cm-diagnostic-warning': { borderLeftColor: 'var(--ansi-3)' },
	'.cm-diagnostic-info': { borderLeftColor: 'var(--ansi-4)' },
	'.cm-diagnostic-hint': { borderLeftColor: 'var(--muted-fg)' }
});

export const highlight = HighlightStyle.define([
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
	{ tag: tags.inserted, color: 'var(--git-added)' },
	{ tag: tags.deleted, color: 'var(--git-deleted)' },
	{ tag: tags.invalid, color: 'var(--ansi-1)' }
]);

export const theme = [chrome, syntaxHighlighting(highlight)];

const removed = (percent: number) => `color-mix(in oklab, var(--git-deleted) calc(${percent}% * var(--diff-tint)), transparent)`;
const added = (percent: number) => `color-mix(in oklab, var(--git-added) calc(${percent}% * var(--diff-tint)), transparent)`;

/** With `whole`, a file added or deleted outright keeps its gutter bars but no tinted lines. */
export const diffTheme = (whole: boolean) => EditorView.theme({
	'&.cm-merge-a .cm-changedLine, & .cm-deletedChunk': { backgroundColor: whole ? 'transparent' : removed(12) },
	'&.cm-merge-b .cm-changedLine, & .cm-inlineChangedLine': { backgroundColor: whole ? 'transparent' : added(12) },
	'&.cm-merge-a .cm-changedText, & .cm-deletedChunk .cm-deletedText, &.cm-merge-b .cm-deletedText': {
		background: removed(8)
	},
	'&.cm-merge-b .cm-changedText': { background: added(8) },
	'&.cm-merge-a .cm-diffWholeLine .cm-changedText, &.cm-merge-b .cm-diffWholeLine .cm-changedText, & .cm-diffWholeLine .cm-changedText, &.cm-merge-a .cm-changedSpace .cm-changedText, &.cm-merge-b .cm-changedSpace .cm-changedText, & .cm-changedSpace .cm-changedText': { background: 'none' },
	'&.cm-merge-a .cm-changedLineGutter, & .cm-deletedLineGutter': { backgroundColor: whole ? 'var(--git-deleted)' : removed(12) },
	'&:not(.cm-merge-a) .cm-changedLineGutter': { backgroundColor: whole ? 'var(--git-added)' : added(12) },
	'& .cm-inlineChangedLineGutter': { backgroundColor: 'var(--git-modified)' },
	'&.cm-merge-a .cm-diffChangedRow:not(.cm-changedLineGutter)': { backgroundColor: whole ? 'transparent' : removed(12) },
	'&:not(.cm-merge-a) .cm-diffChangedRow:not(.cm-changedLineGutter, .cm-inlineChangedLineGutter)': { backgroundColor: whole ? 'transparent' : added(12) },
	'&.cm-merge-a .cm-lineNumbers .cm-diffChangedRow, & .cm-originalNumbers .cm-diffDeletedRow': { color: 'var(--git-deleted)' },
	'&:not(.cm-merge-a) .cm-lineNumbers .cm-diffChangedRow': { color: 'var(--git-added)' },
	'&:not(.cm-focused) .cm-activeLine': { backgroundColor: 'transparent' },
	'&:not(.cm-focused) .cm-activeLineGutter': { color: 'inherit' },
	'&': { '--diff-sign': 'calc(1ch + 2 * var(--space-1))' },
	'& .cm-foldGutter': { display: 'none !important' },
	'& .cm-lineNumbers .cm-gutterElement': { minWidth: 0, paddingInline: 'var(--space-1)' },
	'& .cm-gutter:first-child.cm-lineNumbers .cm-gutterElement': { minWidth: 0, paddingInline: 'var(--space-2) var(--space-1)' },
	'& .cm-originalNumbers .cm-gutterElement': { minWidth: 0, paddingInline: 'var(--space-2) var(--space-1)', textAlign: 'right' },
	'& .cm-diffSigns .cm-gutterElement': { width: 'var(--diff-sign)', paddingInline: 0, fontFamily: 'var(--font-mono)', textAlign: 'center' },
	'@media (pointer: coarse)': { '&': { '--diff-sign': 'var(--space-6)' } },
	'& .cm-diffSigns-a .cm-gutterElement': { color: 'var(--git-deleted)' },
	'& .cm-diffSigns-b .cm-gutterElement': { color: 'var(--git-added)' },
	'& .cm-diffSigns .cm-diffDeletedSign': { color: 'var(--git-deleted)' },
	'& .cm-mergeSpacer': {
		backgroundColor: 'color-mix(in lab, var(--color-bg) 97%, var(--color-fg))',
		backgroundImage: 'repeating-linear-gradient(-45deg, color-mix(in oklab, var(--border) 38%, transparent) 0 var(--border-width), transparent var(--border-width) var(--space-3))'
	},
	'& .cm-diffCollapsedRow': { backgroundColor: 'color-mix(in lab, var(--surface-1) 50%, transparent)' },
	'& .cm-diffDeletedRow': { backgroundColor: whole ? 'transparent' : removed(12) },
	'& .cm-changeGutter': { paddingLeft: 0 },
	'& .cm-collapsedLines': { background: 'color-mix(in lab, var(--surface-1) 50%, transparent)', color: 'var(--muted-fg)', fontFamily: 'var(--font-ui)' },
	'& .cm-collapsedLines::before': { content: '"↕"', marginRight: 'var(--space-2)' },
	'& .cm-collapsedLines::after': { content: 'none' },
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
