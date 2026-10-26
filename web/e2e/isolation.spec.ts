import { expect, test } from './fixtures.ts';
import { writeFileSync } from 'node:fs';
import { openProject } from './shell.ts';

for (const name of ['first', 'second']) {
	test(`${name} test starts with fresh projects and data`, { tag: '@desktop' }, async ({ page }) => {
		writeFileSync(`${process.env.STAMPEDE_E2E_ROOT}/isolation.marker`, 'owned', { flag: 'wx' });
		await page.goto('/');
		await expect(page.getByRole('navigation', { name: 'Projects', includeHidden: true }).getByRole('listitem', { includeHidden: true })).toHaveCount(0);
		await openProject(page, 'alpha');
		await expect(page.getByRole('navigation', { name: 'Projects', includeHidden: true }).getByRole('button', { name: 'alpha', exact: true, includeHidden: true })).toHaveCount(1);
	});
}
