import { commands, type GroupDetail, type GroupSummary } from "#lib/bindings.js";
import { SwrCache } from "./cache.svelte";

/** Group summaries per connection id. */
export const groupsCache = new SwrCache<GroupSummary[]>(() => commands.listGroups(), 15_000);

/** Group details keyed "<connection id>\u0000<group id>". */
export const groupDetailCache = new SwrCache<GroupDetail>(
    (key) => commands.describeGroup(key.split("\u0000")[1]),
    10_000,
);
