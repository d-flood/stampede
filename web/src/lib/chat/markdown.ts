import MarkdownIt from 'markdown-it';
import Check from '~icons/ph/check?raw';
import Copy from '~icons/ph/copy?raw';

const markdown = new MarkdownIt({ breaks: true, linkify: true });
const rules = markdown.renderer.rules;
const copy = `<button class="copy" type="button" aria-label="Copy">${Copy}${Check}</button>`;
const maxCacheBytes = 2 << 20;

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

const inline = rules.code_inline!;
rules.code_inline = (tokens, index, options, env, self) => {
	const html = inline(tokens, index, options, env, self);
	return env.copy && tokens[index].content.length >= 20 ? `<span class="code-inline">${html}${copy}</span>` : html;
};

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
		const html = render(text);
		const size = 2 * (text.length + html.length);
		if (size > maxCacheBytes) return html;
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
