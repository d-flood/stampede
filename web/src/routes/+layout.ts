import { loadConfig } from '#lib/config.svelte.ts';

export const ssr = false;

export const load = () => loadConfig();
