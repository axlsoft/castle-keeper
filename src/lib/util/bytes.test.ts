import { describe, expect, it } from "vitest";
import { fromBase64, hexDump, preview, tryJson, utf8 } from "./bytes";

const raw = (bytes: number[]) => btoa(String.fromCharCode(...bytes));
const b64 = (s: string) => raw([...new TextEncoder().encode(s)]);

describe("bytes", () => {
    it("decodes UTF-8 and flags invalid sequences", () => {
        expect(utf8(fromBase64(b64("héllo")))).toEqual({ text: "héllo", valid: true });
        expect(utf8(fromBase64(raw([0xff, 0xfe, 0x41])))?.valid).toBe(false);
        expect(utf8(fromBase64(null))).toBeNull();
    });

    it("previews on one line and labels binary", () => {
        expect(preview(b64("a\n  b"))).toBe("a b");
        expect(preview(null)).toBe("null");
        expect(preview(raw([0xff, 0x00, 0xfe]))).toMatch(/binary/);
    });

    it("hex-dumps with offsets and ASCII", () => {
        expect(hexDump(fromBase64(b64("AB\n")))).toBe("00000000  41 42 0a" + " ".repeat(39) + "  AB.");
    });

    it("parses JSON objects and arrays only", () => {
        expect(tryJson('{"a":1}')).toEqual({ ok: true, value: { a: 1 } });
        expect(tryJson("42")).toEqual({ ok: false });
        expect(tryJson("{broken")).toEqual({ ok: false });
    });
});
