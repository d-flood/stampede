import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { readFileSync } from 'node:fs';
import Icons from 'unplugin-icons/vite';
import { defineConfig } from 'vite';

const crate = readFileSync(new URL('../crates/stampede/Cargo.toml', import.meta.url), 'utf8');
const brandColors: Record<string, string> = { claude: '#d97757', openai: '#10a37f', githubcopilot: '#8534f3' };

export default defineConfig({
	plugins: [
		{
			// ghostty-web inlines its WASM as base64, which the main thread parses and decodes during startup; the file beside it streams instead.
			name: 'ghostty-wasm-file',
			transform(code, id) {
				if (id.includes('/ghostty-web/dist/ghostty-web.js')) return code.replace(/new URL\("data:application\/wasm;base64,[^"]+", self\.location\)/, 'new URL("./ghostty-vt.wasm", import.meta.url)');
			}
		},
		sveltekit({ adapter: adapter({ fallback: 'index.html', precompress: true }), version: { name: process.env.STAMPEDE_BUILD_ID ?? crate.match(/^version = "(.+)"$/m)?.[1] } }),
		Icons({
			compiler: 'svelte',
			iconCustomizer(collection, icon, props) {
				if (collection === 'simple-icons' && brandColors[icon]) props.color = brandColors[icon];
			}
		})
	]
});
