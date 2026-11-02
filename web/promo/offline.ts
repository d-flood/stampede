export const brief = `# Work offline

Tidepool should keep working with no connection: the app loads offline, edits are queued, and the queue syncs when the network returns.

## Approach
- Cache the app shell with a service worker.
- Record every edit in an IndexedDB outbox before it is applied.
- Replay the outbox when \`online\` fires; the newest edit wins per field.

## Out of scope
Merging edits across devices beyond newest-wins.
`;

export const tickets = [
	{ title: 'Cache the app shell', body: 'Register a service worker that precaches the build and serves it offline.', tags: 'routine', blockers: [], seconds: 3.2 },
	{ title: 'Queue edits in an outbox', body: 'Write each edit to an IndexedDB outbox before saving it.', tags: 'judgment', blockers: [], seconds: 2.2 },
	{ title: 'Show an offline banner', body: 'Tell people when they are offline and how many edits are waiting.', tags: 'frontend-design', blockers: [], seconds: 3.8 },
	{ title: 'Sync the outbox when back online', body: 'Replay queued edits in order when the `online` event fires.', tags: 'judgment', blockers: [2], seconds: 2.6 },
	{ title: 'Resolve conflicting edits', body: 'Keep the newest edit per field when a synced note changed elsewhere.', tags: 'judgment', blockers: [4], seconds: 2.4 }
];
