import MarkdownIt from 'markdown-it';

const markdown = new MarkdownIt({ breaks: true, linkify: true });

markdown.renderer.rules.link_open = (tokens, index, options, _env, self) => {
	tokens[index].attrSet('target', '_blank');
	tokens[index].attrSet('rel', 'noreferrer');
	return self.renderToken(tokens, index, options);
};

const rendered = new Map<string, string>();

export function render(text: string): string {
	const html = rendered.get(text) ?? markdown.render(text);
	rendered.delete(text);
	rendered.set(text, html);
	if (rendered.size > 200) rendered.delete(rendered.keys().next().value!);
	return html;
}
