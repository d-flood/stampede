export function hostOf(projectId: string): string | undefined {
	return projectId.split('@')[1];
}

export function remoteId(projectId: string): string {
	return projectId.split('@')[0];
}

export function projectId(remote: string, host?: string): string {
	return host ? `${remote}@${host}` : remote;
}

export function hostUrl(host: string | undefined, path: string): string {
	return host ? `/cloud/${host}${path}` : path;
}

export function projectUrl(projectId: string, path: string): string {
	return hostUrl(hostOf(projectId), `/api/projects/${remoteId(projectId)}/${path}`);
}
