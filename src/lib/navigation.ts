import type { Component } from "svelte";
import ListTree from "@lucide/svelte/icons/list-tree";
import Users from "@lucide/svelte/icons/users";
import Inbox from "@lucide/svelte/icons/inbox";
import History from "@lucide/svelte/icons/history";
import Plug from "@lucide/svelte/icons/plug";
import Settings from "@lucide/svelte/icons/settings";
import Send from "@lucide/svelte/icons/send";

export interface NavItem {
    href: string;
    label: string;
    icon: Component;
    /** Digit for the ⌘1…⌘6 shortcut. */
    key?: string;
}

export const NAV: NavItem[] = [
    { href: "/topics", label: "Topics", icon: ListTree, key: "1" },
    { href: "/groups", label: "Consumer Groups", icon: Users, key: "2" },
    { href: "/messages", label: "Messages", icon: Inbox, key: "3" },
    { href: "/actions", label: "Action Log", icon: History, key: "4" },
    { href: "/connections", label: "Connections", icon: Plug, key: "5" },
    { href: "/settings", label: "Settings", icon: Settings, key: "6" },
];

/** Pages reachable from the command palette but not the sidebar. */
export const EXTRA_PAGES: NavItem[] = [{ href: "/produce", label: "Produce a message", icon: Send }];
