/** Decodes base64 (as sent by the backend) to bytes. */
export function fromBase64(b64: string | null | undefined): Uint8Array | null {
    if (b64 === null || b64 === undefined) return null;
    const binary = atob(b64);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
    return bytes;
}

const strict = new TextDecoder("utf-8", { fatal: true });
const lenient = new TextDecoder("utf-8");

export interface DecodedText {
    text: string;
    /** False when the bytes aren't valid UTF-8 (text then has U+FFFD). */
    valid: boolean;
}

export function utf8(bytes: Uint8Array | null): DecodedText | null {
    if (!bytes) return null;
    try {
        return { text: strict.decode(bytes), valid: true };
    } catch {
        return { text: lenient.decode(bytes), valid: false };
    }
}

/** Single-line preview for table cells. */
export function preview(b64: string | null | undefined, max = 200): string {
    if (b64 === null || b64 === undefined) return "null";
    const decoded = utf8(fromBase64(b64.length > max * 2 ? b64.slice(0, Math.ceil((max * 4) / 3 / 4) * 4) : b64));
    if (!decoded) return "";
    if (!decoded.valid) return `‹${Math.floor((b64.length * 3) / 4)} bytes binary›`;
    return decoded.text.replace(/\s+/g, " ").slice(0, max);
}

/** Classic hex dump: offset, 16 hex bytes, ASCII. */
export function hexDump(bytes: Uint8Array | null, limit = 64 * 1024): string {
    if (!bytes) return "";
    const lines: string[] = [];
    const end = Math.min(bytes.length, limit);
    for (let offset = 0; offset < end; offset += 16) {
        const chunk = bytes.subarray(offset, Math.min(offset + 16, end));
        const hex = Array.from(chunk, (b) => b.toString(16).padStart(2, "0")).join(" ");
        const ascii = Array.from(chunk, (b) => (b >= 0x20 && b < 0x7f ? String.fromCharCode(b) : ".")).join("");
        lines.push(`${offset.toString(16).padStart(8, "0")}  ${hex.padEnd(47)}  ${ascii}`);
    }
    if (bytes.length > end) lines.push(`… ${bytes.length - end} more bytes`);
    return lines.join("\n");
}

export type JsonParse = { ok: true; value: unknown } | { ok: false };

export function tryJson(text: string | undefined | null): JsonParse {
    if (!text) return { ok: false };
    const trimmed = text.trimStart();
    if (!(trimmed.startsWith("{") || trimmed.startsWith("["))) return { ok: false };
    try {
        return { ok: true, value: JSON.parse(text) };
    } catch {
        return { ok: false };
    }
}
