import { image } from '#lib/files.svelte.ts';
import { LanguageDescription, LanguageSupport, StreamLanguage, type StreamParser } from '@codemirror/language';
import type { Extension } from '@codemirror/state';
import File from '~icons/ph/file?raw';
import FileImage from '~icons/ph/file-image?raw';
import FilePdf from '~icons/ph/file-pdf?raw';
import GitDiff from '~icons/ph/git-diff?raw';
import Astro from '~icons/simple-icons/astro?raw';
import Css from '~icons/simple-icons/css?raw';
import Docker from '~icons/simple-icons/docker?raw';
import Bash from '~icons/simple-icons/gnubash?raw';
import Html from '~icons/simple-icons/html5?raw';
import JavaScript from '~icons/simple-icons/javascript?raw';
import Json from '~icons/simple-icons/json?raw';
import Markdown from '~icons/simple-icons/markdown?raw';
import Python from '~icons/simple-icons/python?raw';
import React from '~icons/simple-icons/react?raw';
import Rust from '~icons/simple-icons/rust?raw';
import Svelte from '~icons/simple-icons/svelte?raw';
import Svg from '~icons/simple-icons/svg?raw';
import Toml from '~icons/simple-icons/toml?raw';
import TypeScript from '~icons/simple-icons/typescript?raw';
import Vue from '~icons/simple-icons/vuedotjs?raw';
import Xml from '~icons/simple-icons/xml?raw';
import Yaml from '~icons/simple-icons/yaml?raw';

type Load = () => Promise<LanguageSupport>;
type Language = { extensions: string[]; filenames?: string[]; icon: string; color?: string; load: Load; servers?: string[] };

const stream = <T>(parser: StreamParser<T>) => new LanguageSupport(StreamLanguage.define(parser));
const [red, green, yellow, blue, magenta, cyan] = [1, 2, 3, 4, 5, 6].map((slot) => `var(--ansi-${slot})`);

const javascript = (options: { typescript?: boolean; jsx?: boolean }): Load => async () =>
	(await import('@codemirror/lang-javascript')).javascript(options);
const markdown: Load = async () => {
	const [{ markdown, markdownLanguage }, { yamlFrontmatter }, { footnotes }] = await Promise.all([
		import('@codemirror/lang-markdown'),
		import('@codemirror/lang-yaml'),
		import('./live-preview.ts')
	]);
	return yamlFrontmatter({ content: markdown({ base: markdownLanguage, codeLanguages, extensions: [footnotes] }) });
};
const xml: Load = async () => stream((await import('@codemirror/legacy-modes/mode/xml')).xml);
const shell: Load = async () => {
	const { shell } = await import('@codemirror/legacy-modes/mode/shell');
	return stream({
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

// Keys double as LSP language IDs for entries with servers.
const languages: Record<string, Language> = {
	typescript: { extensions: ['ts', 'mts', 'cts'], icon: TypeScript, color: blue, load: javascript({ typescript: true }), servers: ['typescript', 'eslint'] },
	typescriptreact: { extensions: ['tsx'], icon: React, color: cyan, load: javascript({ typescript: true, jsx: true }), servers: ['typescript', 'eslint'] },
	javascript: { extensions: ['js', 'mjs', 'cjs'], icon: JavaScript, color: yellow, load: javascript({}), servers: ['typescript', 'eslint'] },
	javascriptreact: { extensions: ['jsx'], icon: React, color: cyan, load: javascript({ jsx: true }), servers: ['typescript', 'eslint'] },
	python: { extensions: ['py', 'pyi'], icon: Python, color: blue, load: async () => (await import('@codemirror/lang-python')).python(), servers: ['python', 'ruff'] },
	css: { extensions: ['css'], icon: Css, color: magenta, load: async () => (await import('@codemirror/lang-css')).css() },
	html: { extensions: ['html'], icon: Html, color: red, load: async () => (await import('@codemirror/lang-html')).html() },
	svg: { extensions: ['svg'], icon: Svg, color: yellow, load: xml },
	xml: { extensions: ['xml'], icon: Xml, color: blue, load: xml },
	json: { extensions: ['json', 'jsonc', 'jsonl', 'jsonld', 'webmanifest'], icon: Json, color: yellow, load: async () => (await import('@codemirror/lang-json')).json() },
	markdown: { extensions: ['md', 'markdown'], icon: Markdown, load: markdown },
	rust: { extensions: ['rs'], icon: Rust, load: async () => (await import('@codemirror/lang-rust')).rust() },
	yaml: { extensions: ['yaml', 'yml'], icon: Yaml, color: red, load: async () => (await import('@codemirror/lang-yaml')).yaml() },
	shell: { extensions: ['sh', 'bash', 'zsh'], icon: Bash, color: green, load: shell },
	toml: { extensions: ['toml'], icon: Toml, color: red, load: async () => stream((await import('@codemirror/legacy-modes/mode/toml')).toml) },
	dockerfile: {
		extensions: ['dockerfile'],
		filenames: ['Dockerfile'],
		icon: Docker,
		color: blue,
		load: async () => stream((await import('@codemirror/legacy-modes/mode/dockerfile')).dockerFile)
	},
	diff: { extensions: ['diff', 'patch'], icon: GitDiff, load: async () => stream((await import('@codemirror/legacy-modes/mode/diff')).diff) },
	svelte: { extensions: ['svelte'], icon: Svelte, color: red, load: async () => (await import('@replit/codemirror-lang-svelte')).svelte(), servers: ['svelte', 'eslint'] },
	astro: { extensions: ['astro'], icon: Astro, color: magenta, load: async () => (await import('./astro.ts')).astro(), servers: ['astro', 'eslint'] },
	vue: { extensions: ['vue'], icon: Vue, color: green, load: async () => (await import('@codemirror/lang-vue')).vue() }
};

const lookup = new Map(
	Object.entries(languages).flatMap(([name, { extensions, filenames = [] }]) => [
		...extensions.map((extension) => [`.${extension}`, name] as const),
		...filenames.map((filename) => [filename, name] as const)
	])
);

const codeLanguages = Object.entries(languages).map(([name, { extensions, load }]) => LanguageDescription.of({ name, alias: extensions, load }));

export function languageOf(path: string): string | undefined {
	const name = path.slice(path.lastIndexOf('/') + 1);
	return lookup.get(name) ?? lookup.get(name.match(/\.[^.]+$/)?.[0].toLowerCase() ?? '');
}

export function fenceLanguage(info: string): string | undefined {
	return LanguageDescription.matchLanguageName(codeLanguages, info)?.name;
}

export function syntax(name: string | undefined): Promise<Extension> {
	return name ? languages[name].load() : Promise.resolve([]);
}

export function language(path: string): Promise<Extension> {
	return syntax(languageOf(path));
}

export function languageServers(path: string): [servers: string[], languageId: string] | undefined {
	const name = languageOf(path);
	const servers = name && languages[name].servers;
	return name && servers ? [servers, name] : undefined;
}

export function fileIcon(path: string): { svg: string; color?: string } {
	const name = languageOf(path);
	if (name) return { svg: languages[name].icon, color: languages[name].color };
	return { svg: image(path) ? FileImage : /\.pdf$/i.test(path) ? FilePdf : File };
}
