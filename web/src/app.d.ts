/// <reference types="unplugin-icons/types/svelte5" />

import type { Terminal } from 'ghostty-web';

declare global {
	namespace App {
		interface PageState {
			level?: import('#lib/phone.svelte.ts').Level;
			depth?: number;
		}
	}

	interface Window {
		terminal?: Terminal;
	}
}

declare module 'svelte/elements' {
	interface HTMLTextareaAttributes {
		autocorrect?: 'on' | 'off' | null;
	}
}

export {};
