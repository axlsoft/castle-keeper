#!/usr/bin/env bash
# Seeds the local compose broker with realistic data for development:
# topics with varied partition counts, a compacted topic, JSON payloads with
# headers, a lagging consumer group and two caught-up (empty) groups.
#
# Runs the Kafka CLI inside the `kafka` compose service, so it only ever
# touches the local development broker. Safe to re-run: topics are created
# if missing, messages are appended, and group offsets are reset.
set -euo pipefail

cd "$(dirname "$0")/.."

compose() {
  if docker compose version >/dev/null 2>&1; then docker compose "$@"; else docker-compose "$@"; fi
}

kafka() {
  compose exec -T kafka bash -c "$1"
}

BOOTSTRAP=localhost:9092
BIN=/opt/kafka/bin

echo "Waiting for the broker…"
compose up -d --wait kafka >/dev/null

create_topic() { # name partitions [extra args…]
  local name=$1 partitions=$2
  shift 2
  kafka "$BIN/kafka-topics.sh --bootstrap-server $BOOTSTRAP --create --if-not-exists \
    --topic $name --partitions $partitions --replication-factor 1 $*" >/dev/null
  echo "  topic $name ($partitions partitions)"
}

echo "Creating topics…"
create_topic orders 6 --config retention.ms=604800000
create_topic payments 3
create_topic customers 3 --config cleanup.policy=compact --config min.cleanable.dirty.ratio=0.1
create_topic audit.log 1 --config retention.ms=2592000000
create_topic metrics.raw 12
create_topic dlq.orders 1

# Produces lines "headers<TAB>key<TAB>value" generated inside the container.
produce() { # topic count generator-body
  local topic=$1 count=$2 body=$3
  kafka "for i in \$(seq 1 $count); do $body; done | $BIN/kafka-console-producer.sh \
    --bootstrap-server $BOOTSTRAP --topic $topic \
    --property parse.key=true --property parse.headers=true \
    --property 'key.separator=	' --property 'headers.delimiter=	'" >/dev/null
  echo "  $count messages -> $topic"
}

echo "Producing messages…"
produce orders 600 'printf "source:storefront,trace-id:t%05d\torder-%d\t{\"orderId\":%d,\"customerId\":\"c%03d\",\"status\":\"%s\",\"total\":%d.%02d,\"items\":[{\"sku\":\"SKU-%03d\",\"qty\":%d}],\"createdAt\":\"2026-10-%02dT12:%02d:00Z\"}\n" $i $i $i $((i % 97)) $([ $((i % 5)) -eq 0 ] && echo cancelled || echo placed) $((i % 300 + 10)) $((i % 100)) $((i % 250)) $((i % 4 + 1)) $((i % 28 + 1)) $((i % 60))'
produce payments 300 'printf "source:payments-api\tpay-%d\t{\"paymentId\":\"pay-%d\",\"orderId\":%d,\"amount\":%d.%02d,\"currency\":\"%s\",\"method\":\"card\"}\n" $i $i $((i * 2)) $((i % 500 + 5)) $((i % 100)) $([ $((i % 3)) -eq 0 ] && echo EUR || echo USD)'
produce customers 200 'printf "source:crm\tc%03d\t{\"customerId\":\"c%03d\",\"name\":\"Customer %d\",\"tier\":\"%s\",\"version\":%d}\n" $((i % 97)) $((i % 97)) $((i % 97)) $([ $((i % 7)) -eq 0 ] && echo gold || echo standard) $i'
produce audit.log 100 'printf "source:auth\tuser-%d\tuser-%d signed in from 10.0.%d.%d\n" $((i % 20)) $((i % 20)) $((i % 255)) $((i % 200))'
produce metrics.raw 1200 'printf "source:agent\thost-%02d\t{\"host\":\"host-%02d\",\"cpu\":%d.%d,\"ts\":%d}\n" $((i % 40)) $((i % 40)) $((i % 100)) $((i % 10)) $((1760000000 + i))'
produce dlq.orders 12 'printf "source:order-worker,error:validation-failed\torder-%d\t{\"orderId\":%d,\"error\":\"missing customerId\"}\n" $((i * 37)) $((i * 37))'

# Group offsets are set directly; none of these groups has live members.
set_offsets() { # group topic how
  kafka "$BIN/kafka-consumer-groups.sh --bootstrap-server $BOOTSTRAP --group $1 --topic $2 \
    --reset-offsets $3 --execute" >/dev/null
}

echo "Creating consumer groups…"
# billing-service: about half way through orders -> lagging.
set_offsets billing-service orders "--to-earliest"
set_offsets billing-service orders "--shift-by 50"
echo "  billing-service (lagging on orders)"
# analytics: fully caught up on payments and orders.
set_offsets analytics payments "--to-latest"
set_offsets analytics orders "--to-latest"
echo "  analytics (caught up)"
# reporting: caught up on customers, no members -> Empty with zero lag.
set_offsets reporting customers "--to-latest"
echo "  reporting (empty, caught up)"

echo "Done. Connect to localhost:9092."
