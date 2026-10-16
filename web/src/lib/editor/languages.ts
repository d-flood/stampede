import { StreamLanguage } from '@codemirror/language';
import type { Extension } from '@codemirror/state';

type Load = () => Promise<Extension>;

const typescript: Load = async () => (await import('@codemirror/lang-javascript')).javascript({ typescript: true });
const javascript: Load = async () => (await import('@codemirror/lang-javascript')).javascript();
const python: Load = async () => (await import('@codemirror/lang-python')).python();
const markdown: Load = async () => (await import('@codemirror/lang-markdown')).markdown();
const yaml: Load = async () => (await import('@codemirror/lang-yaml')).yaml();

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
	json: async () => (await import('@codemirror/lang-json')).json(),
	md: markdown,
	markdown,
	rs: async () => (await import('@codemirror/lang-rust')).rust(),
	yaml,
	yml: yaml,
	toml: async () => StreamLanguage.define((await import('@codemirror/legacy-modes/mode/toml')).toml),
	svelte: async () => (await import('@replit/codemirror-lang-svelte')).svelte()
};

export function language(path: string): Promise<Extension> {
	const extension = path.match(/\.([^./]+)$/)?.[1].toLowerCase() ?? '';
	return loaders[extension]?.() ?? Promise.resolve([]);
}
