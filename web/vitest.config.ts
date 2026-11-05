import { svelte } from '@sveltejs/vite-plugin-svelte';
import { playwright } from '@vitest/browser-playwright';
import { devices } from '@playwright/test';
import Icons from 'unplugin-icons/vite';
import { defineConfig } from 'vitest/config';
import { checkHardwareGpu, hardwareLaunchOptions } from './e2e/gpu.ts';

if (!process.env.STAMPEDE_E2E_ROOT) throw new Error('Run Vitest through ./check components');

export default defineConfig({
	plugins: [svelte(), Icons({ compiler: 'svelte' })],
	publicDir: 'static',
	optimizeDeps: {
		include: [
			'@codemirror/lang-css', '@codemirror/lang-html', '@codemirror/lang-javascript', '@codemirror/lang-json',
			'@codemirror/lang-markdown', '@codemirror/lang-python', '@codemirror/lang-rust', '@codemirror/lang-vue',
			'@codemirror/lang-yaml', '@codemirror/language', '@codemirror/state', '@codemirror/view', '@lezer/highlight',
			'@codemirror/legacy-modes/mode/diff', '@codemirror/legacy-modes/mode/dockerfile', '@codemirror/legacy-modes/mode/shell',
			'@codemirror/legacy-modes/mode/toml', '@codemirror/legacy-modes/mode/xml', '@replit/codemirror-lang-svelte', 'bits-ui', 'fuzzysort', 'style-mod'
		]
	},
	test: {
		api: { host: '127.0.0.1', port: 0, strictPort: true },
		maxWorkers: 3,
		reporters: ['default', 'json'],
		outputFile: { json: '../target/check/vitest-results.json' },
		projects: [
			{ extends: true, test: { name: 'unit', include: ['tests/**/*.test.ts'], environment: 'node' } },
			{
				extends: true,
				test: {
					name: 'components',
					include: ['components/**/*.test.ts'],
					setupFiles: ['components/setup.ts'],
					browser: {
						enabled: true,
						headless: true,
						provider: playwright({ launchOptions: hardwareLaunchOptions, contextOptions: { reducedMotion: 'reduce', permissions: ['clipboard-read', 'clipboard-write'] } }),
						instances: [
							{ browser: 'chromium', name: 'desktop-components', viewport: { width: 1280, height: 800 } },
							{ browser: 'chromium', name: 'pixel-components', viewport: { width: 448, height: 998 }, provider: playwright({ launchOptions: hardwareLaunchOptions, contextOptions: { ...devices['Pixel 8 Pro'], reducedMotion: 'reduce', permissions: ['clipboard-read', 'clipboard-write'] } }) }
						],
						commands: {
							async hardwareGpu({ context }) {
								const browser = context.browser();
								if (!browser) throw new Error('Chromium browser is unavailable');
								await checkHardwareGpu(browser);
							}
						}
					}
				}
			}
		]
	}
});
