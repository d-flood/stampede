/// <reference types="unplugin-icons/types/svelte5" />

import type { Terminal } from 'ghostty-web';

declare global {
	namespace App {}

	interface Window {
		terminal?: Terminal;
	}
}

export {};
