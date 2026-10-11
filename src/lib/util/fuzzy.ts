/**
 * Subsequence fuzzy match. Returns a score (higher is better) or null when
 * `query` is not a subsequence of `text`. Consecutive runs, word starts and
 * an early first match score higher.
 */
export function fuzzyScore(query: string, text: string): number | null {
    if (!query) return 0;
    const q = query.toLowerCase();
    const t = text.toLowerCase();
    const exact = t.indexOf(q);
    if (exact >= 0) return 1000 - exact + (exact === 0 ? 200 : 0);

    let score = 0;
    let ti = 0;
    let run = 0;
    let last = -2;
    for (let qi = 0; qi < q.length; qi++) {
        const ch = q[qi];
        let found = -1;
        for (; ti < t.length; ti++) {
            if (t[ti] === ch) {
                found = ti;
                break;
            }
        }
        if (found < 0) return null;
        const prev = found > 0 ? t[found - 1] : "";
        const wordStart = found === 0 || prev === "." || prev === "-" || prev === "_" || prev === "/";
        run = found === last + 1 ? run + 1 : 0;
        last = found;
        score += 10 + run * 15 + (wordStart ? 20 : 0) - Math.min(found, 20) * 0.1;
        ti = found + 1;
    }
    return score;
}

/** Filters and sorts `items` by fuzzy score of `key(item)`. */
export function fuzzyFilter<T>(items: T[], query: string, key: (item: T) => string): T[] {
    const trimmed = query.trim();
    if (!trimmed) return items;
    const scored: { item: T; score: number }[] = [];
    for (const item of items) {
        const score = fuzzyScore(trimmed, key(item));
        if (score !== null) scored.push({ item, score });
    }
    scored.sort((a, b) => b.score - a.score);
    return scored.map((s) => s.item);
}
