import { describe, expect, it } from "vitest";
import type { Profile } from "./bindings";
import { PASSWORD_ENV, browseCommands, groupCommands, quote, topicCommands } from "./cli";

const base: Profile = {
    id: "p1",
    name: "local",
    environment: "local",
    bootstrap: "localhost:9092",
    sasl: null,
    tls: { enabled: false, caPath: null, certPath: null, keyPath: null, verifyHostname: true },
    readOnly: false,
    adminTimeoutMs: 10000,
    deliveryTimeoutMs: 15000,
};

const scram: Profile = {
    ...base,
    bootstrap: "b1:9093,b2:9093",
    sasl: { mechanism: "scramSha512", username: "alice" },
    tls: { enabled: true, caPath: "/etc/ca.pem", certPath: null, keyPath: null, verifyHostname: true },
};

describe("quote", () => {
    it("leaves simple tokens bare and single-quotes the rest", () => {
        expect(quote("orders.v1")).toBe("orders.v1");
        expect(quote("my topic")).toBe("'my topic'");
        expect(quote("it's")).toBe(`'it'"'"'s'`);
    });
});

describe("plaintext profile", () => {
    it("uses --bootstrap-server without a config file", () => {
        const [describeCmd, , kcat] = topicCommands(base, "orders");
        expect(describeCmd.command).toBe("kafka-topics.sh --bootstrap-server localhost:9092 --describe --topic orders");
        expect(kcat.command).toBe("kcat -b localhost:9092 -L -t orders");
    });
});

describe("SASL/SCRAM over TLS", () => {
    it("never includes a secret and references the env var instead", () => {
        const all = [...topicCommands(scram, "orders"), ...groupCommands(scram, "billing")]
            .map((c) => c.command)
            .join("\n");
        expect(all).not.toMatch(/alice-secret/);
        expect(all).toContain(`\${${PASSWORD_ENV}}`);
    });

    it("writes Java client properties through an expanding heredoc", () => {
        const cmd = groupCommands(scram, "billing")[0].command;
        expect(cmd).toContain("cat > /tmp/castle-keeper-client.properties <<EOF");
        expect(cmd).toContain("security.protocol=SASL_SSL");
        expect(cmd).toContain("sasl.mechanism=SCRAM-SHA-512");
        expect(cmd).toContain("ssl.truststore.type=PEM");
        expect(cmd).toContain("--command-config /tmp/castle-keeper-client.properties");
        expect(cmd.split("\n").at(-1)).toMatch(/^kafka-consumer-groups.sh --bootstrap-server b1:9093,b2:9093 --describe --group billing/);
    });

    it("passes librdkafka properties to kcat with the password left to the shell", () => {
        const kcat = topicCommands(scram, "orders")[2].command;
        expect(kcat).toContain("-X security.protocol=SASL_SSL");
        expect(kcat).toContain("-X sasl.mechanisms=SCRAM-SHA-512");
        expect(kcat).toContain(`-X "sasl.password=$${PASSWORD_ENV}"`);
        expect(kcat).toContain("-X ssl.ca.location=/etc/ca.pem");
    });
});

describe("browse", () => {
    const request = (start: Parameters<typeof browseCommands>[1]["start"], partitions: number[] = []) => ({
        topic: "orders",
        partitions,
        start,
        limit: 100,
    });

    it("maps each start position to kcat's -o", () => {
        expect(browseCommands(base, request({ kind: "earliest" }))[0].command).toContain("-o beginning");
        expect(browseCommands(base, request({ kind: "latest", count: 50 }))[0].command).toContain("-o -50");
        expect(browseCommands(base, request({ kind: "offset", offset: 42 }))[0].command).toContain("-o 42");
        expect(browseCommands(base, request({ kind: "timestamp", timestamp: 1700000000000 }))[0].command).toContain(
            "-o s@1700000000000",
        );
    });

    it("emits one command per selected partition and stops at the end", () => {
        const cmd = browseCommands(base, request({ kind: "earliest" }, [0, 2]))[0].command;
        expect(cmd.split("\n")).toHaveLength(2);
        expect(cmd).toContain("-p 0");
        expect(cmd).toContain("-p 2");
        expect(cmd).toContain(" -c 100 -e ");
    });
});
