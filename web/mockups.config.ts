import { defineConfig } from '@playwright/test';
import base from './playwright.config.ts';

export default defineConfig({
	testDir: 'e2e',
	testMatch: 'mockups.ts',
	use: base.use,
	webServer: base.webServer
});
