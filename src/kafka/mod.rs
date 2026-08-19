pub mod back_worker;
pub mod front_consumer;

use rdkafka::error::{KafkaError, RDKafkaErrorCode};

pub(super) fn topic_is_not_available(error: &KafkaError) -> bool {
    matches!(
        error,
        KafkaError::MessageConsumption(RDKafkaErrorCode::UnknownTopicOrPartition)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_missing_topic_as_temporary_consumption_error() {
        assert!(topic_is_not_available(&KafkaError::MessageConsumption(
            RDKafkaErrorCode::UnknownTopicOrPartition,
        )));
        assert!(!topic_is_not_available(&KafkaError::MessageConsumption(
            RDKafkaErrorCode::AllBrokersDown,
        )));
    }
}
