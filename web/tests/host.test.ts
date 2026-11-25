import { hostOf, hostUrl, projectId, projectUrl, remoteId } from '#lib/host.ts';
import { expect, test } from 'vitest';

test('Project ids and URLs distinguish their Hosts', () => {
	const remote = 'ab12cd34ef56';
	const host = 'stampede-1a2b3c4d-x9y8z7';
	for (const [owner, id, prefix] of [
		[undefined, remote, ''],
		[host, `${remote}@${host}`, `/cloud/${host}`]
	] as const) {
		expect(projectId(remote, owner)).toBe(id);
		expect(hostOf(id)).toBe(owner);
		expect(remoteId(id)).toBe(remote);
		expect(hostUrl(owner, '/api/x')).toBe(`${prefix}/api/x`);
		expect(projectUrl(id, 'file?path=a+b')).toBe(`${prefix}/api/projects/${remote}/file?path=a+b`);
	}
});
