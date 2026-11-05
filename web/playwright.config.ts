import { defineConfig, devices } from '@playwright/test';
import { hardwareLaunchOptions } from './e2e/gpu.ts';

if (!process.env.STAMPEDE_E2E_ROOT || !process.env.STAMPEDE_E2E_BINARY) {
	throw new Error('Run Playwright through ./check e2e');
}

const desktop = { ...devices['Desktop Chrome'], viewport: { width: 1280, height: 800 } };
const pixel = { ...devices['Pixel 8 Pro'], viewport: { width: 448, height: 998 } };
const performance = process.env.STAMPEDE_CHECK_PERF === '1';

export default defineConfig({
	testDir: 'e2e',
	fullyParallel: !performance,
	workers: performance ? 1 : 3,
	reporter: [['dot'], ['json', { outputFile: performance ? '../target/check/e2e-perf-results.json' : '../target/check/e2e-results.json' }]],
	use: { launchOptions: hardwareLaunchOptions, reducedMotion: 'reduce' },
	projects: performance ? [
		{ name: 'desktop-perf', testMatch: 'perf.spec.ts', use: desktop }
	] : [
		{ name: 'desktop', grepInvert: /@phone/, testIgnore: 'perf.spec.ts', use: desktop },
		{ name: 'pixel', grepInvert: /@desktop/, testIgnore: 'perf.spec.ts', use: pixel }
	]
});
