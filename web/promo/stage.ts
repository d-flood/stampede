import { expect, type Page } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { cpSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { Chat } from './chat.ts';

export const seed = fileURLToPath(new URL('../../promo/seed/tidepool/', import.meta.url));

const commits: [string, string[]][] = [
	['Scaffold SvelteKit app', ['package.json', 'svelte.config.js', 'vite.config.ts', 'tsconfig.json', '.gitignore', 'src/app.html', 'src/app.css', 'src/routes/+layout.svelte', 'static', 'scripts']],
	['Store notes in IndexedDB', ['src/lib/notes.ts', 'src/lib/notes.test.ts']],
	['Add note list and editor', ['src/lib/components', 'src/routes/+page.svelte']],
	['Write README', ['README.md']]
];

export function project(root: string, name: string) {
	const path = join(root, 'projects', name);
	cpSync(seed, path, { recursive: true });
	const git = (...args: string[]) => execFileSync('git', ['-C', path, ...args]);
	git('init', '-q', '-b', 'main');
	git('config', 'user.name', 'Ada Lin');
	git('config', 'user.email', 'ada@example.com');
	git('config', 'commit.gpgsign', 'false');
	for (const [message, paths] of commits) {
		git('add', ...paths);
		git('commit', '-q', '-m', message);
	}
	return path;
}

export type Rig = { name: string; fixture: string; agent?: 'claude-code' | 'codex'; model?: string; effort?: string; mode?: string; shortcut?: string; gates?: string };

export async function configure(page: Page, root: string, rigs: Rig[], { commit = 'Add tags to notes\n\nStore tags on each note, add them from the editor, and filter the list by tag.', codex = 'simple-turn' } = {}) {
	const draft = join(root, 'draft.jsonl');
	new Chat(seed).prompt().sleep(1.2).end(commit).write(draft);
	const usage = join(root, 'codex-usage.json');
	const hour = Math.floor(Date.now() / 1000) + 3600;
	const window = (usedPercent: number, windowDurationMins: number, resetsAt: number) => ({ usedPercent, windowDurationMins, resetsAt });
	writeFileSync(usage, JSON.stringify({ usage: { ordinaryUsageAllowed: true, rateLimits: { limitId: 'codex', primary: window(38, 300, hour * 1 + 7200), secondary: window(24, 10080, hour + 300_000), credits: { hasCredits: false, unlimited: false, balance: '0' }, planType: 'pro' }, rateLimitResetCredits: { credits: null } } }));
	const toml = (fields: Record<string, string | undefined>) => Object.entries(fields).filter(([, value]) => value).map(([key, value]) => `${key} = ${JSON.stringify(value)}`);
	const rig = (rig: Rig) => {
		const agent = rig.agent ?? 'claude-code';
		const home = agent === 'codex' ? { STAMPEDE_FAKE_RESETS: usage, CODEX_HOME: join(root, 'codex') } : { CLAUDE_CONFIG_DIR: join(root, 'claude') };
		const env = toml({ STAMPEDE_FAKE_FIXTURE: rig.fixture, ...home, STAMPEDE_FAKE_GATES: rig.gates });
		const mode = rig.mode ?? (agent === 'codex' ? undefined : 'default');
		return `\n[[rig]]\n${[...toml({ name: rig.name, agent, model: rig.model, effort: rig.effort, mode, shortcut: rig.shortcut }), `env = { ${env.join(', ')} }`].join('\n')}\n`;
	};
	const all = [...rigs, { name: 'Claude Haiku', fixture: draft }, { name: 'Codex', agent: 'codex' as const, fixture: codex }];
	writeFileSync(join(root, 'config.toml'), `name = "Studio"
roots = [${JSON.stringify(join(root, 'projects'))}]
commit_rig = "Claude Haiku"
${all.map(rig).join('')}`);
	await expect.poll(async () => (await (await page.request.get('/api/rigs')).json()).map((rig: { name: string }) => rig.name)).toEqual(all.map((rig) => rig.name));
}
