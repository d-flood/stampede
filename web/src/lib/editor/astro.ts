import { html } from '@codemirror/lang-html';
import { typescriptLanguage } from '@codemirror/lang-javascript';
import { LanguageSupport, LRLanguage } from '@codemirror/language';
import { parseMixed, type Input } from '@lezer/common';

const typescript = { parser: typescriptLanguage.parser };

function frontmatter(input: Input) {
	const match = /^---[^\S\n]*\n([\s\S]*?\n)?---[^\S\n]*(?:\n|$)/.exec(input.read(0, input.length));
	if (!match) return null;
	const from = match[0].indexOf('\n') + 1;
	return { ...typescript, overlay: [{ from, to: from + (match[1]?.length ?? 0) }] };
}

function expressions(text: string, offset: number) {
	const ranges = [];
	for (let at = text.indexOf('{'); at >= 0; at = text.indexOf('{', at)) {
		let depth = 0;
		let end = at;
		while (end < text.length) {
			if (text[end] === '{') depth++;
			else if (text[end] === '}' && !--depth) break;
			end++;
		}
		ranges.push({ from: offset + at, to: offset + Math.min(end + 1, text.length) });
		at = end + 1;
	}
	return ranges.length ? { ...typescript, overlay: ranges } : null;
}

export function astro() {
	const base = html({
		selfClosingTags: true,
		nestedLanguages: [{ tag: 'script', attrs: (attrs) => !attrs.type || attrs.type === 'module', parser: typescriptLanguage.parser }]
	});
	const language = (base.language as LRLanguage).configure(
		{
			wrap: parseMixed((node, input) => {
				switch (node.name) {
					case 'Document':
						return frontmatter(input);
					case 'Text':
						return expressions(input.read(node.from, node.to), node.from);
					case 'AttributeName':
					case 'UnquotedAttributeValue':
						return input.read(node.from, node.from + 1) === '{' ? typescript : null;
				}
				return null;
			})
		},
		'astro'
	);
	return new LanguageSupport(language, base.support);
}
