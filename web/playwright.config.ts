import { defineConfig, devices } from '@playwright/test';

const port = process.env.OIKONOMOS_E2E_PORT;
const root = process.env.OIKONOMOS_E2E_ROOT;
if (!port || !root) throw new Error('Run Playwright through ./check e2e');

export default defineConfig({
	testDir: 'e2e',
	use: { baseURL: `http://127.0.0.1:${port}` },
	projects: [
		{
			name: 'desktop',
			use: { ...devices['Desktop Chrome'], viewport: { width: 1280, height: 800 } }
		},
		{
			name: 'pixel',
			use: { ...devices['Pixel 8 Pro'], viewport: { width: 448, height: 998 } }
		}
	],
	webServer: {
		command: `../target/release/oikonomos serve --port ${port} --data-dir "${root}/data" --config "${root}/config.toml"`,
		url: `http://127.0.0.1:${port}/api/version`
	}
});
