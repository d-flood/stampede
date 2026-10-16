export function count(tokens: number): string {
	if (tokens < 1000) return String(tokens);
	if (tokens < 999_500) return `${Math.round(tokens / 1000)}k`;
	return `${Number((tokens / 1_000_000).toFixed(1))}M`;
}
