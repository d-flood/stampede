import type { Browser, LaunchOptions } from '@playwright/test';

export const hardwareLaunchOptions = {
	channel: 'chromium',
	ignoreDefaultArgs: ['--enable-unsafe-swiftshader'],
	args: [
		'--enable-gpu',
		'--enable-gpu-rasterization',
		'--disable-software-rasterizer',
		'--disable-software-compositing-fallback',
		...(process.platform === 'linux' ? ['--use-angle=vulkan'] : [])
	]
} satisfies LaunchOptions;

export function requireHardwareGpu(gpu: {
	devices: { vendorId: number; deviceId: number; deviceString?: string; driverVendor?: string }[];
	featureStatus?: Record<string, string>;
	auxAttributes?: Record<string, unknown>;
}) {
	const device = gpu.devices[0];
	const software = /swiftshader|swangle|llvmpipe|softpipe|lavapipe|software|subzero/i;
	if (!device || !device.vendorId || device.vendorId === 0xffff || software.test(JSON.stringify(gpu.devices)) ||
		software.test(String(gpu.auxAttributes?.glRenderer ?? '')) ||
		['gpu_compositing', 'rasterization', 'webgl'].some((feature) => !/^enabled(?:_force|_on)?$/.test(gpu.featureStatus?.[feature] ?? ''))) {
		throw new Error(`E2E requires hardware GPU compositing, rasterization and WebGL. Software rendering is forbidden. Run outside the sandbox with access to the GPU.\n${JSON.stringify(gpu, null, 2)}`);
	}
}

export async function checkHardwareGpu(browser: Browser) {
	const session = await browser.newBrowserCDPSession();
	try {
		const { gpu } = await session.send('SystemInfo.getInfo');
		requireHardwareGpu(gpu);
	} finally {
		await session.detach();
	}
}
