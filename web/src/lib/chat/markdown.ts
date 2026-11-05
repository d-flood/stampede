import { codeLines, fenceParser } from '#lib/chat/highlight.ts';
import { highlight } from '#lib/editor/theme.ts';
import MarkdownIt from 'markdown-it';
import { StyleModule } from 'style-mod';
import Check from '~icons/ph/check?raw';
import Copy from '~icons/ph/copy?raw';

const markdown = new MarkdownIt({ breaks: true, linkify: true, highlight: highlighted });
const rules = markdown.renderer.rules;
const copy = `<button class="copy" type="button" aria-label="Copy">${Copy}${Check}</button>`;
const maxCacheBytes = 2 << 20;
let loading = false;

if (highlight.module) StyleModule.mount(document, highlight.module);

function highlighted(code: string, info: string): string {
	const syntax = fenceParser(info);
	if (syntax === 'loading') loading = true;
	if (!syntax || syntax === 'loading') return '';
	const escape = markdown.utils.escapeHtml;
	return codeLines(code, syntax)
		.map((line) => line.map(({ text, classes }) => (classes ? `<span class="${classes}">${escape(text)}</span>` : escape(text))).join(''))
		.join('\n');
}

rules.link_open = (tokens, index, options, _env, self) => {
	tokens[index].attrSet('target', '_blank');
	tokens[index].attrSet('rel', 'noreferrer');
	return self.renderToken(tokens, index, options);
};

for (const name of ['fence', 'code_block'] as const) {
	const rule = rules[name]!;
	rules[name] = (tokens, index, options, env, self) => {
		const html = rule(tokens, index, options, env, self);
		return env.copy ? `<div class="code-block">${html}${copy}</div>` : html;
	};
}

rules.table_open = (tokens, index, options, env, self) => (env.copy ? '<div class="table">' : '') + self.renderToken(tokens, index, options);
rules.table_close = (tokens, index, options, env, self) => self.renderToken(tokens, index, options) + (env.copy ? '</div>' : '');

const inline = rules.code_inline!;
rules.code_inline = (tokens, index, options, env, self) => {
	const html = inline(tokens, index, options, env, self);
	return env.copy && tokens[index].content.length >= 20 ? `<span class="code-inline">${html}${copy}</span>` : html;
};

// CSS can't size a column by its text length, and without a floor long text shrinks to its longest word.
markdown.core.ruler.push('column_widths', ({ tokens }) => {
	const longest: number[] = [];
	let table = 0;
	let column = 0;
	tokens.forEach((token, index) => {
		if (token.type === 'table_open') {
			table = index;
			longest.length = 0;
		}
		if (token.type === 'tr_open') column = 0;
		if (token.type === 'th_open' || token.type === 'td_open') {
			const length = tokens[index + 1].children!.reduce((sum, child) => sum + child.content.length, 0);
			longest[column] = Math.max(longest[column] ?? 0, length);
			column++;
		}
		if (token.type === 'table_close') {
			tokens.slice(table, index).filter(({ type }) => type === 'th_open').forEach((header, position) => {
				const width = Math.round(Math.min(longest[position] / 5, 24));
				header.attrSet('style', `${header.attrGet('style') ?? ''};min-width:${width}ch`);
			});
		}
	});
});

function cached(render: (text: string) => string) {
	const rendered = new Map<string, string>();
	let bytes = 0;
	return (text: string, cache = true) => {
		if (!cache) return render(text);
		const existing = rendered.get(text);
		if (existing !== undefined) {
			rendered.delete(text);
			rendered.set(text, existing);
			return existing;
		}
		loading = false;
		const html = render(text);
		const size = 2 * (text.length + html.length);
		if (loading || size > maxCacheBytes) return html;
		rendered.set(text, html);
		bytes += size;
		while (rendered.size > 200 || bytes > maxCacheBytes) {
			const [key, value] = rendered.entries().next().value!;
			bytes -= 2 * (key.length + value.length);
			rendered.delete(key);
		}
		return html;
	};
}

export const render = cached((text) => markdown.render(text));
export const renderCopyable = cached((text) => markdown.render(text, { copy: true }));
