import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { readFileSync } from 'node:fs';
import Icons from 'unplugin-icons/vite';
import { defineConfig } from 'vite';

const crate = readFileSync(new URL('../crates/oikonomos/Cargo.toml', import.meta.url), 'utf8');

export default defineConfig({
	plugins: [
		sveltekit({ adapter: adapter({ fallback: 'index.html' }), version: { name: process.env.OIKONOMOS_BUILD_ID ?? crate.match(/^version = "(.+)"$/m)?.[1] } }),
		Icons({ compiler: 'svelte' })
	]
});
