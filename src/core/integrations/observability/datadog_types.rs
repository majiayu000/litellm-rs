use serde::Serialize;
use std::collections::VecDeque;

#[derive(Debug, Clone, Serialize)]
pub(super) struct MetricPoint {
    pub(super) timestamp: i64,
    pub(super) value: f64,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct MetricSeries {
    pub(super) metric: String,
    #[serde(rename = "type")]
    pub(super) metric_type: i32,
    pub(super) points: Vec<MetricPoint>,
    pub(super) tags: Vec<String>,
    pub(super) unit: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct MetricsPayload {
    pub(super) series: Vec<MetricSeries>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct DataDogLogRecord {
    pub(super) ddsource: String,
    pub(super) ddtags: String,
    pub(super) hostname: String,
    pub(super) message: String,
    pub(super) service: String,
    pub(super) status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) timestamp: Option<i64>,
}

#[derive(Debug, Clone)]
pub(super) enum BufferedEvent {
    Metric(MetricSeries),
    Log(DataDogLogRecord),
}

#[derive(Debug, Default)]
pub(super) struct EventBuffer {
    pub(super) pending: VecDeque<BufferedEvent>,
    pub(super) in_flight: Vec<BufferedEvent>,
}

impl EventBuffer {
    pub(super) fn len(&self) -> usize {
        self.pending.len() + self.in_flight.len()
    }
}
