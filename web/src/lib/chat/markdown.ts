import MarkdownIt from 'markdown-it';

const markdown = new MarkdownIt({ breaks: true, linkify: true });

markdown.renderer.rules.link_open = (tokens, index, options, _env, self) => {
	tokens[index].attrSet('target', '_blank');
	tokens[index].attrSet('rel', 'noreferrer');
	return self.renderToken(tokens, index, options);
};

export function render(text: string): string {
	return markdown.render(text);
}
