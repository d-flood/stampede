import { loadConfig } from '#lib/config.svelte.ts';
import { preconnect } from '#lib/device.svelte.ts';

export const ssr = false;

preconnect();
// The router asks for the page code only after it starts; fetching it now overlaps the download with startup.
void import('./+page.svelte');

export const load = () => loadConfig();
