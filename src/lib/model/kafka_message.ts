export interface KafkaMessageRequest {
    key: string;
    value: string;
}

export interface KafkaMessageResponse {
    partition: number;
    offset: number;
}
