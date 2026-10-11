import { describe, expect, it } from "vitest";
import { fuzzyFilter, fuzzyScore } from "./fuzzy";

describe("fuzzyScore", () => {
    it("matches subsequences and rejects non-matches", () => {
        expect(fuzzyScore("ordv", "orders.v2")).not.toBeNull();
        expect(fuzzyScore("xyz", "orders")).toBeNull();
        expect(fuzzyScore("", "anything")).toBe(0);
    });

    it("ranks exact prefixes above scattered matches", () => {
        const names = ["svc.orders.audit", "orders", "o-r-d-e-r-s"];
        expect(fuzzyFilter(names, "orders", (n) => n)[0]).toBe("orders");
    });

    it("is case-insensitive", () => {
        expect(fuzzyFilter(["Payments", "orders"], "PAY", (n) => n)).toEqual(["Payments"]);
    });
});
