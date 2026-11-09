import { astro } from '#lib/editor/astro.ts';
import { classHighlighter, highlightTree } from '@lezer/highlight';
import { expect, test } from 'vitest';

const source = `---
import Layout from '../layouts/Layout.astro';
const { title } = Astro.props;
const items: Array<string> = ['a'];
---
<Layout title={title}>
	<h1 class="big">{title.toUpperCase()}</h1>
	<ul>{items.map((item) => <li>{item}</li>)}</ul>
	<Card {title} />
</Layout>
<script>
	const count: number = 1;
</script>
`;

function tokens(code: string) {
	const found: [string, string][] = [];
	highlightTree(astro().language.parser.parse(code), classHighlighter, (from, to, classes) => {
		found.push([code.slice(from, to), classes]);
	});
	return found;
}

test('Astro highlights frontmatter, expressions, attribute expressions, and scripts as TypeScript', () => {
	const found = tokens(source);
	const classes = (text: string) => found.filter(([token]) => token === text).map(([, classes]) => classes);
	expect(classes('import')).toEqual(['tok-keyword']);
	expect(classes('Array')).toEqual(['tok-typeName']);
	expect(classes('string')).toEqual(['tok-typeName']);
	expect(classes('number')).toEqual(['tok-typeName']);
	expect(classes('toUpperCase')).toEqual(['tok-propertyName']);
	expect(classes('items')).toEqual(['tok-variableName tok-definition', 'tok-variableName']);
	expect(classes('title')).toContain('tok-propertyName');
	expect(classes('title').filter((name) => name === 'tok-variableName').length).toBeGreaterThanOrEqual(3);
	expect(classes('Layout')).toContain('tok-typeName');
	expect(classes('h1')).toEqual(['tok-typeName', 'tok-typeName']);
	expect(classes('"big"')).toEqual(['tok-string']);
});

test('Astro without frontmatter stays HTML', () => {
	const found = tokens('<p>{1 + 2}</p>\n');
	expect(found).toContainEqual(['p', 'tok-typeName']);
	expect(found).toContainEqual(['1', 'tok-number']);
});
