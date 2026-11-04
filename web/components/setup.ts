import '@fontsource-variable/literata/opsz-italic.css';
import '@fontsource-variable/literata/opsz.css';
import '@fontsource/fira-sans/400.css';
import '@fontsource/fira-sans/600.css';
import '#lib/tokens.css';
import './style.css';
import { afterAll, afterEach, beforeAll, vi } from 'vitest';
import { commands } from 'vitest/browser';
import { cleanup } from 'vitest-browser-svelte';

declare module 'vitest/browser' {
	interface BrowserCommands {
		hardwareGpu(): Promise<void>;
	}
}

beforeAll(async () => {
	await commands.hardwareGpu();
	await Promise.all(['16px "Literata Variable"', '16px "Fira Sans"', '16px "JetBrains Mono"'].map((font) => document.fonts.load(font)));
	document.body.classList.toggle('phone', matchMedia('(pointer: coarse) and (max-width: 767px)').matches);
});

afterEach(async () => {
	await cleanup();
	vi.useRealTimers();
	vi.restoreAllMocks();
});

afterAll(() => commands.hardwareGpu());
