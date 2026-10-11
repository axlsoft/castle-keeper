import { commands, type TopicSummary } from "@/bindings";
import { SwrCache } from "./cache.svelte";

/**
 * All topics (including internal ones) per connection id; views filter.
 * The key is the connection id so switching connections never shows
 * another cluster's topics.
 */
export const topicsCache = new SwrCache<TopicSummary[]>(() => commands.listTopics(true));
