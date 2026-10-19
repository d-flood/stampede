import { expect, test as base } from '@playwright/test';
import { execFileSync, spawn } from 'node:child_process';
import { closeSync, mkdirSync, mkdtempSync, openSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { checkHardwareGpu } from './gpu.ts';

export { expect } from '@playwright/test';

const repository = fileURLToPath(new URL('../../', import.meta.url));
const fixtures = join(repository, 'scripts/check_fixtures.sh');
const cleanup = join(repository, 'scripts/check_cleanup.py');
const fakes = join(repository, 'crates/oikonomos/tests/fake');

export const test = base.extend<{ hardwareGpu: void; starterRigs: boolean }>({
	starterRigs: [false, { option: true }],
	baseURL: async ({ starterRigs }, use, testInfo) => {
		const parent = process.env.OIKONOMOS_E2E_ROOT!;
		const root = mkdtempSync(join(parent, 'test-'));
		try {
			const port = execFileSync('bash', [fixtures, root], { encoding: 'utf8' }).trim();
			if (starterRigs) writeFileSync(join(root, 'config.toml'), `roots = [${JSON.stringify(join(root, 'projects'))}]\n`);
			process.env.OIKONOMOS_E2E_ROOT = root;
			const log = testInfo.outputPath('server.log');
			mkdirSync(dirname(log), { recursive: true });
			const output = openSync(log, 'w');
			const server = spawn(process.env.OIKONOMOS_E2E_BINARY!, [
				'serve', '--port', port, '--data-dir', join(root, 'data'), '--config', join(root, 'config.toml')
			], { env: {
				...process.env,
				PATH: `${fakes}:${process.env.PATH}`,
				...(starterRigs ? { OIKONOMOS_FAKE_FIXTURE: 'simple-turn', CLAUDE_CONFIG_DIR: join(root, 'claude'), CODEX_HOME: join(root, 'codex') } : {})
			}, stdio: ['ignore', output, output] });
			closeSync(output);
			const url = `http://127.0.0.1:${port}`;
			await expect.poll(async () => {
				if (server.exitCode !== null) throw new Error(`Test server exited: ${server.exitCode}; see ${log}`);
				try { return (await fetch(`${url}/api/version`)).ok; }
				catch { return false; }
			}, { timeout: 10_000, intervals: [25, 50, 100] }).toBe(true);
			await use(url);
		} finally {
			process.env.OIKONOMOS_E2E_ROOT = parent;
			execFileSync('python3', [cleanup, root]);
			rmSync(root, { recursive: true, force: true });
		}
	},
	hardwareGpu: [async ({ browser }, use) => {
		await checkHardwareGpu(browser);
		await use();
		await checkHardwareGpu(browser);
	}, { auto: true }]
});
