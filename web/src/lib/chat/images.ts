export function pastedImages(event: ClipboardEvent) {
	return [...(event.clipboardData?.items ?? [])]
		.filter((item) => item.kind === 'file' && item.type.startsWith('image/'))
		.map((item) => item.getAsFile())
		.filter((file) => file !== null);
}
