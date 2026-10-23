import { StreamLanguage } from '@codemirror/language';
import type { Extension } from '@codemirror/state';

type Load = () => Promise<Extension>;

const typescript: Load = async () => (await import('@codemirror/lang-javascript')).javascript({ typescript: true });
const javascript: Load = async () => (await import('@codemirror/lang-javascript')).javascript();
const python: Load = async () => (await import('@codemirror/lang-python')).python();
const markdown: Load = async () => (await import('@codemirror/lang-markdown')).markdown();
const yaml: Load = async () => (await import('@codemirror/lang-yaml')).yaml();
const json: Load = async () => (await import('@codemirror/lang-json')).json();
const xml: Load = async () => StreamLanguage.define((await import('@codemirror/legacy-modes/mode/xml')).xml);
const diff: Load = async () => StreamLanguage.define((await import('@codemirror/legacy-modes/mode/diff')).diff);
const shell: Load = async () => {
	const { shell } = await import('@codemirror/legacy-modes/mode/shell');
	return StreamLanguage.define({
		...shell,
		token(stream, state) {
			const style = shell.token(stream, state);
			if (style === 'builtin') return 'variableName.function';
			if (style) return style;
			if (/^[|&;()<>]+$/.test(stream.current())) return 'operator';
			if (/^[\w./-]+$/.test(stream.current()) && /(?:^|[|;&(])\s*$/.test(stream.string.slice(0, stream.start))) {
				stream.eatWhile(/[\w./-]/);
				return 'variableName.function';
			}
			return null;
		}
	});
};

const loaders: Record<string, Load> = {
	ts: typescript,
	mts: typescript,
	cts: typescript,
	tsx: async () => (await import('@codemirror/lang-javascript')).javascript({ typescript: true, jsx: true }),
	js: javascript,
	mjs: javascript,
	cjs: javascript,
	jsx: async () => (await import('@codemirror/lang-javascript')).javascript({ jsx: true }),
	py: python,
	pyi: python,
	css: async () => (await import('@codemirror/lang-css')).css(),
	html: async () => (await import('@codemirror/lang-html')).html(),
	svg: xml,
	xml,
	json,
	jsonc: json,
	jsonl: json,
	jsonld: json,
	webmanifest: json,
	md: markdown,
	markdown,
	rs: async () => (await import('@codemirror/lang-rust')).rust(),
	yaml,
	yml: yaml,
	sh: shell,
	bash: shell,
	zsh: shell,
	toml: async () => StreamLanguage.define((await import('@codemirror/legacy-modes/mode/toml')).toml),
	dockerfile: async () => StreamLanguage.define((await import('@codemirror/legacy-modes/mode/dockerfile')).dockerFile),
	diff,
	patch: diff,
	svelte: async () => (await import('@replit/codemirror-lang-svelte')).svelte(),
	vue: async () => (await import('@codemirror/lang-vue')).vue()
};

export function extension(path: string): string {
	if (/(?:^|\/)Dockerfile$/.test(path)) return 'dockerfile';
	return path.match(/\.([^./]+)$/)?.[1].toLowerCase() ?? '';
}

export function language(path: string): Promise<Extension> {
	return loaders[extension(path)]?.() ?? Promise.resolve([]);
}
