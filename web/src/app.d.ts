/// <reference types="unplugin-icons/types/svelte5" />

import type { Terminal } from 'ghostty-web';

declare global {
	namespace App {}

	interface Window {
		terminal?: Terminal;
	}
}

declare module '~icons/*?raw' {
	const svg: string;
	export default svg;
}

declare module 'svelte/elements' {
	interface HTMLTextareaAttributes {
		autocorrect?: 'on' | 'off' | null;
	}
}

export {};
