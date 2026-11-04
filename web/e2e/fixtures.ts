import { expect, test as base } from '../tests/browser.ts';
import { execFileSync, spawn } from 'node:child_process';
import { closeSync, cpSync, mkdirSync, mkdtempSync, openSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:net';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

export { expect } from '@playwright/test';

const repository = fileURLToPath(new URL('../../', import.meta.url));
const fixtures = join(repository, 'scripts/check_fixtures.sh');
const cleanup = join(repository, 'scripts/check_cleanup.py');
const fakes = join(repository, 'crates/stampede/tests/fake');
const shell = join(repository, 'web/e2e/bash');

async function unusedPort() {
	const server = createServer();
	await new Promise<void>((resolve, reject) => {
		server.once('error', reject);
		server.listen(0, '127.0.0.1', resolve);
	});
	const address = server.address();
	await new Promise<void>((resolve, reject) => server.close((error) => error ? reject(error) : resolve()));
	if (!address || typeof address === 'string') throw new Error('Could not allocate a test port');
	return address.port;
}

export const test = base.extend<{ starterRigs: boolean }, { projectTemplate: string }>({
	starterRigs: [false, { option: true }],
	projectTemplate: [async ({}, use) => {
		const template = mkdtempSync(join(process.env.STAMPEDE_E2E_ROOT!, 'template-'));
		try {
			execFileSync('bash', [fixtures, template]);
			await use(template);
		} finally {
			rmSync(template, { recursive: true, force: true });
		}
	}, { scope: 'worker' }],
	baseURL: async ({ starterRigs, projectTemplate }, use, testInfo) => {
		const parent = process.env.STAMPEDE_E2E_ROOT!;
		const root = mkdtempSync(join(parent, 'test-'));
		try {
			cpSync(projectTemplate, root, { recursive: true });
			writeFileSync(join(root, 'config.toml'), readFileSync(join(root, 'config.toml'), 'utf8').replaceAll(projectTemplate, root));
			const port = String(await unusedPort());
			if (starterRigs) writeFileSync(join(root, 'config.toml'), `roots = [${JSON.stringify(join(root, 'projects'))}]\n`);
			process.env.STAMPEDE_E2E_ROOT = root;
			const log = testInfo.outputPath('server.log');
			mkdirSync(dirname(log), { recursive: true });
			const output = openSync(log, 'w');
			const server = spawn(process.env.STAMPEDE_E2E_BINARY!, [
				'serve', '--port', port, '--data-dir', join(root, 'data'), '--config', join(root, 'config.toml')
			], { env: {
				...process.env,
				CLAUDE_CODE_ENABLE_SDK_FILE_CHECKPOINTING: undefined,
				SHELL: shell,
				PATH: `${fakes}:${process.env.PATH}`,
				...(starterRigs ? { STAMPEDE_FAKE_FIXTURE: 'simple-turn', CLAUDE_CONFIG_DIR: join(root, 'claude'), CODEX_HOME: join(root, 'codex') } : {})
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
			process.env.STAMPEDE_E2E_ROOT = parent;
			execFileSync('python3', [cleanup, root]);
			rmSync(root, { recursive: true, force: true });
		}
	}
});
