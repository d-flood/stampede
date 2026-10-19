import { defineConfig, devices } from '@playwright/test';
import { hardwareLaunchOptions } from './e2e/gpu.ts';

if (!process.env.STAMPEDE_E2E_ROOT || !process.env.STAMPEDE_E2E_BINARY) {
	throw new Error('Run Playwright through ./check e2e');
}

export default defineConfig({
	testDir: 'e2e',
	fullyParallel: true,
	workers: 2,
	reporter: 'dot',
	use: { launchOptions: hardwareLaunchOptions },
	projects: [
		{
			name: 'desktop',
			use: { ...devices['Desktop Chrome'], viewport: { width: 1280, height: 800 } }
		},
		{
			name: 'pixel',
			use: { ...devices['Pixel 8 Pro'], viewport: { width: 448, height: 998 } }
		}
	]
});
