import { test as base } from '@playwright/test';
import { checkHardwareGpu } from '../e2e/gpu.ts';

export { expect } from '@playwright/test';

export const test = base.extend<{}, { hardwareGpu: void }>({
	hardwareGpu: [async ({ browser }, use) => {
		await checkHardwareGpu(browser);
		await use();
		await checkHardwareGpu(browser);
	}, { auto: true, scope: 'worker' }]
});
