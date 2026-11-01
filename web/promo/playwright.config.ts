import { defineConfig } from '@playwright/test';
import { hardwareLaunchOptions } from '../e2e/gpu.ts';

if (!process.env.STAMPEDE_E2E_ROOT || !process.env.STAMPEDE_E2E_BINARY) {
	throw new Error('Run promo captures through ./check e2e --config promo/playwright.config.ts');
}

export default defineConfig({
	testDir: '.',
	outputDir: '../test-results/promo',
	workers: 1,
	reporter: 'list',
	use: { launchOptions: hardwareLaunchOptions },
	projects: [{ name: 'promo' }]
});
