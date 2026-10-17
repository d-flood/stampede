export class OpenBrowser {
	url = $state('');

	constructor(public projectId: string, public id = `browser:${crypto.randomUUID()}`) {}

	get name() {
		return this.url ? new URL(this.url).host : 'Browser';
	}
}
