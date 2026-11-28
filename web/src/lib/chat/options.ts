export type Options = { models: { value: string; label: string; efforts: string[] }[]; settings?: { model?: string; effort?: string } };

/** Listing models starts the provider's CLI, so each Rig asks once per session, and only when someone reaches for the chips. */
const listings = new Map<string, Promise<Options>>();

export function listOptions(agent: string, rig: string) {
	const key = `${agent}:${rig}`;
	let listing = listings.get(key);
	if (!listing) {
		listing = fetch(`/api/rigs/options?${new URLSearchParams({ agent, rig })}`).then((response) => (response.ok ? response.json() : Promise.reject(new Error('options'))));
		listing.catch(() => listings.delete(key));
		listings.set(key, listing);
	}
	return listing;
}
