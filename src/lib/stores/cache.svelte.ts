import { toAppError, type AppError } from "@/ipc-service";

interface Entry<T> {
    data: T | null;
    fetchedAt: number | null;
    loading: boolean;
    error: AppError | null;
}

/**
 * Stale-while-revalidate cache keyed by connection (and optionally a
 * resource key). Read with `entry` (pure, safe in `$derived`); call `ensure`
 * from an `$effect` to fetch when missing or stale; `refresh` forces a fetch.
 */
export class SwrCache<T> {
    // Raw (not deeply proxied): entries are replaced whole, and large lists
    // stay cheap to store and read.
    #entries = $state.raw<Record<string, Entry<T>>>({});
    #inflight = new Map<string, Promise<void>>();

    constructor(
        private fetcher: (key: string) => Promise<T>,
        private maxAgeMs = 30_000,
    ) {}

    entry(key: string): Entry<T> {
        return this.#entries[key] ?? { data: null, fetchedAt: null, loading: false, error: null };
    }

    /** Fetches `key` when it has no data yet or is older than maxAge. */
    ensure(key: string) {
        const entry = this.entry(key);
        const stale = entry.fetchedAt === null || Date.now() - entry.fetchedAt > this.maxAgeMs;
        if (stale && !this.#inflight.has(key)) void this.refresh(key);
    }

    refresh(key: string): Promise<void> {
        const existing = this.#inflight.get(key);
        if (existing) return existing;
        const previous = this.entry(key);
        this.#set(key, { ...previous, loading: true });
        const run = this.fetcher(key)
            .then((data) => {
                this.#set(key, { data, fetchedAt: Date.now(), loading: false, error: null });
            })
            .catch((e) => {
                this.#set(key, { ...this.entry(key), loading: false, error: toAppError(e) });
            })
            .finally(() => this.#inflight.delete(key));
        this.#inflight.set(key, run);
        return run;
    }

    #set(key: string, entry: Entry<T>) {
        this.#entries = { ...this.#entries, [key]: entry };
    }

    /** Drops every entry, e.g. after the active connection changed. */
    clear() {
        this.#entries = {};
    }
}

export function ago(timestamp: number | null, now = Date.now()): string {
    if (timestamp === null) return "never";
    const seconds = Math.round((now - timestamp) / 1000);
    if (seconds < 5) return "just now";
    if (seconds < 60) return `${seconds}s ago`;
    const minutes = Math.round(seconds / 60);
    return minutes < 60 ? `${minutes}m ago` : `${Math.round(minutes / 60)}h ago`;
}
