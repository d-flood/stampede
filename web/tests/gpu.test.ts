import assert from 'node:assert/strict';
import { test } from 'node:test';
import { readdirSync, readFileSync } from 'node:fs';
import { hardwareLaunchOptions, requireHardwareGpu } from '../e2e/gpu.ts';

const hardware = () => ({
	devices: [{ vendorId: 0x8086, deviceId: 0x9a49, deviceString: 'Intel Iris Xe', driverVendor: 'Mesa' }],
	featureStatus: { gpu_compositing: 'enabled', rasterization: 'enabled_force', webgl: 'enabled' }
});

test('hardware rasterization is accepted', () => {
	requireHardwareGpu(hardware());
});

test('software rasterization or compositing is rejected', () => {
	for (const feature of ['rasterization', 'gpu_compositing', 'webgl']) {
		const gpu = hardware();
		gpu.featureStatus[feature as keyof typeof gpu.featureStatus] = 'disabled_software';
		assert.throws(() => requireHardwareGpu(gpu), /Software rendering is forbidden/);
	}
});

test('software devices are rejected even when features report enabled', () => {
	for (const deviceString of ['SwiftShader Device (Subzero)', 'llvmpipe', 'softpipe', 'lavapipe', 'Software Rasterizer']) {
		const gpu = hardware();
		gpu.devices[0].deviceString = deviceString;
		assert.throws(() => requireHardwareGpu(gpu), /Software rendering is forbidden/);
	}
});

test('missing devices and incomplete feature reports fail closed', () => {
	assert.throws(() => requireHardwareGpu({ ...hardware(), devices: [] }));
	assert.throws(() => requireHardwareGpu({ ...hardware(), featureStatus: {} }));
});

test('Chromium launch disables the SwiftShader fallback', () => {
	assert.equal(hardwareLaunchOptions.channel, 'chromium');
	assert.ok(hardwareLaunchOptions.ignoreDefaultArgs?.includes('--enable-unsafe-swiftshader'));
	assert.ok(hardwareLaunchOptions.args?.includes('--disable-software-rasterizer'));
	assert.ok(hardwareLaunchOptions.args?.includes('--disable-software-compositing-fallback'));
});


test('every E2E spec uses the GPU-enforcing fixture', () => {
	const directory = new URL('../e2e/', import.meta.url);
	for (const name of readdirSync(directory).filter((name) => name.endsWith('.spec.ts'))) {
		const source = readFileSync(new URL(name, directory), 'utf8');
		assert.match(source, /import \{ expect, test \} from '\.\/fixtures\.ts'/, name);
	}
});
