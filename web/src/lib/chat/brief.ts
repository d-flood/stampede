export function brief(text: string) {
	return text
		.replace(/(?:\/home|\/Users)\/[^/\s'"]+(?=[/\s'"]|$)/g, '~')
		.replace(/[^\s'"]*(?:\/[^\s/'"]+){5,}/g, (path) => {
			const parts = path.split('/');
			return [...parts.slice(0, 2), '…', ...parts.slice(-2)].join('/');
		});
}
