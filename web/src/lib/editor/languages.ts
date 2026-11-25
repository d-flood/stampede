import { image } from '#lib/files.svelte.ts';
import { LanguageDescription, LanguageSupport, StreamLanguage, type StreamParser } from '@codemirror/language';
import type { Extension } from '@codemirror/state';
import File from '~icons/ph/file?raw';
import FileCode from '~icons/ph/file-code?raw';
import FileCss from '~icons/ph/file-css?raw';
import FileHtml from '~icons/ph/file-html?raw';
import FileImage from '~icons/ph/file-image?raw';
import FileIni from '~icons/ph/file-ini?raw';
import FileJs from '~icons/ph/file-js?raw';
import FileJsx from '~icons/ph/file-jsx?raw';
import FileMd from '~icons/ph/file-md?raw';
import FilePdf from '~icons/ph/file-pdf?raw';
import FilePy from '~icons/ph/file-py?raw';
import FileRs from '~icons/ph/file-rs?raw';
import FileSvg from '~icons/ph/file-svg?raw';
import FileTs from '~icons/ph/file-ts?raw';
import FileTsx from '~icons/ph/file-tsx?raw';
import FileVue from '~icons/ph/file-vue?raw';
import GitDiff from '~icons/ph/git-diff?raw';

type Load = () => Promise<LanguageSupport>;
type Language = { extensions: string[]; filenames?: string[]; icon: string; load: Load; servers?: string[] };

const stream = <T>(parser: StreamParser<T>) => new LanguageSupport(StreamLanguage.define(parser));

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
	typescript: { extensions: ['ts', 'mts', 'cts'], icon: FileTs, load: javascript({ typescript: true }), servers: ['typescript', 'eslint'] },
	typescriptreact: { extensions: ['tsx'], icon: FileTsx, load: javascript({ typescript: true, jsx: true }), servers: ['typescript', 'eslint'] },
	javascript: { extensions: ['js', 'mjs', 'cjs'], icon: FileJs, load: javascript({}), servers: ['typescript', 'eslint'] },
	javascriptreact: { extensions: ['jsx'], icon: FileJsx, load: javascript({ jsx: true }), servers: ['typescript', 'eslint'] },
	python: { extensions: ['py', 'pyi'], icon: FilePy, load: async () => (await import('@codemirror/lang-python')).python(), servers: ['python', 'ruff'] },
	css: { extensions: ['css'], icon: FileCss, load: async () => (await import('@codemirror/lang-css')).css() },
	html: { extensions: ['html'], icon: FileHtml, load: async () => (await import('@codemirror/lang-html')).html() },
	svg: { extensions: ['svg'], icon: FileSvg, load: xml },
	xml: { extensions: ['xml'], icon: FileCode, load: xml },
	json: { extensions: ['json', 'jsonc', 'jsonl', 'jsonld', 'webmanifest'], icon: FileCode, load: async () => (await import('@codemirror/lang-json')).json() },
	markdown: { extensions: ['md', 'markdown'], icon: FileMd, load: markdown },
	rust: { extensions: ['rs'], icon: FileRs, load: async () => (await import('@codemirror/lang-rust')).rust() },
	yaml: { extensions: ['yaml', 'yml'], icon: FileIni, load: async () => (await import('@codemirror/lang-yaml')).yaml() },
	shell: { extensions: ['sh', 'bash', 'zsh'], icon: FileCode, load: shell },
	toml: { extensions: ['toml'], icon: FileIni, load: async () => stream((await import('@codemirror/legacy-modes/mode/toml')).toml) },
	dockerfile: {
		extensions: ['dockerfile'],
		filenames: ['Dockerfile'],
		icon: FileCode,
		load: async () => stream((await import('@codemirror/legacy-modes/mode/dockerfile')).dockerFile)
	},
	diff: { extensions: ['diff', 'patch'], icon: GitDiff, load: async () => stream((await import('@codemirror/legacy-modes/mode/diff')).diff) },
	svelte: { extensions: ['svelte'], icon: FileCode, load: async () => (await import('@replit/codemirror-lang-svelte')).svelte(), servers: ['svelte', 'eslint'] },
	astro: { extensions: ['astro'], icon: FileCode, load: async () => (await import('./astro.ts')).astro(), servers: ['astro', 'eslint'] },
	vue: { extensions: ['vue'], icon: FileVue, load: async () => (await import('@codemirror/lang-vue')).vue() }
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

export function fileIcon(path: string): string {
	const name = languageOf(path);
	if (name) return languages[name].icon;
	return image(path) ? FileImage : /\.pdf$/i.test(path) ? FilePdf : File;
}
