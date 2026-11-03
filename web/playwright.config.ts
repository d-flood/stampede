import { defineConfig, devices } from '@playwright/test';
import { hardwareLaunchOptions } from './e2e/gpu.ts';

if (!process.env.STAMPEDE_E2E_ROOT || !process.env.STAMPEDE_E2E_BINARY) {
	throw new Error('Run Playwright through ./check e2e');
}

const desktop = { ...devices['Desktop Chrome'], viewport: { width: 1280, height: 800 } };
const pixel = { ...devices['Pixel 8 Pro'], viewport: { width: 448, height: 998 } };

export default defineConfig({
	testDir: 'e2e',
	fullyParallel: true,
	workers: 3,
	reporter: 'dot',
	use: { launchOptions: hardwareLaunchOptions },
	projects: [
		{ name: 'desktop', grepInvert: /@phone/, testIgnore: 'perf.spec.ts', use: desktop },
		{ name: 'pixel', grepInvert: /@desktop/, testIgnore: 'perf.spec.ts', use: pixel },
		// Timings only mean something on an otherwise idle machine, so perf runs after everything else, one test at a time.
		{ name: 'desktop-perf', grepInvert: /@phone/, testMatch: 'perf.spec.ts', fullyParallel: false, dependencies: ['desktop', 'pixel'], use: desktop },
		{ name: 'pixel-perf', grepInvert: /@desktop/, testMatch: 'perf.spec.ts', fullyParallel: false, dependencies: ['desktop-perf'], use: pixel }
	]
});
