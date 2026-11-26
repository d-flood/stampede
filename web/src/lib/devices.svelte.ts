import { connect, establish, type Device, type ServerMessage } from '#lib/device.svelte.ts';
import { hostOf } from '#lib/host.ts';

const LINGER = 30_000;

/** One Device per Host. A Cloud Host's is open only while shown, since its connection keeps the Cloud Host awake. */
export class Devices {
	readonly home = connect();
	#remotes = new Map<string, Device>();
	#closing = new Map<string, ReturnType<typeof setTimeout>>();
	#message: (message: ServerMessage, host: string) => void;

	constructor(message: (message: ServerMessage, host: string) => void) {
		this.#message = message;
	}

	/** Call outside reactions: Svelte leaves state created in one, as a Device's is, out of its dependencies. */
	list(hosts: Set<string>) {
		for (const host of hosts) {
			if (this.#remotes.has(host)) continue;
			const device = establish(host);
			device.listen({ message: (message) => this.#message(message, host) });
			this.#remotes.set(host, device);
		}
		for (const [host, device] of this.#remotes) if (!hosts.has(host)) device.close();
	}

	get hosts(): Iterable<string> {
		return this.#remotes.keys();
	}

	of(projectId: string): Device {
		return this.at(hostOf(projectId));
	}

	at(host?: string): Device {
		return host ? this.#remotes.get(host)! : this.home;
	}

	/** Opens a Cloud Host's Device, to close some time after the returned function is called. */
	show(host: string): () => void {
		clearTimeout(this.#closing.get(host));
		this.at(host).open();
		return () => this.#closing.set(host, setTimeout(() => this.at(host).close(), LINGER));
	}

	close() {
		for (const closing of this.#closing.values()) clearTimeout(closing);
		for (const device of this.#remotes.values()) device.close();
		this.home.close();
	}
}
