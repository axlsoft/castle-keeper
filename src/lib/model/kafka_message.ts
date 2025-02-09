export enum KafkaMessageType {
    Primitive = 'Primitive',
    Json = 'Json',
    Avro = 'Avro',
    Protobuf = 'Protobuf',
}

export interface MessageMetadata {
    message_type: KafkaMessageType;
}

export interface KafkaMessageRequest {
    key: string;
    value: string;
    metadata: MessageMetadata;
}

export interface KafkaMessageResponse {
    partition: number,
    offset: number,
}