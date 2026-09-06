use std::collections::BTreeMap;
use std::sync::Arc;

use bytes::Bytes;
use tokio::sync::mpsc;

use crate::core::guardrails::{CheckResult, GuardrailAction, GuardrailEngine};
use crate::server::guardrails::GuardrailDecisionSink;

const CONTEXT_OVERLAP_CHARS: usize = 4096;
const MAX_PENDING_BYTES: usize = 1024 * 1024;
const MAX_SURFACES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StreamGuardrailError {
    Violation,
    Execution,
}

impl StreamGuardrailError {
    pub(super) fn error_type(self) -> &'static str {
        match self {
            Self::Violation => "content_policy_error",
            Self::Execution => "server_error",
        }
    }

    pub(super) fn code(self) -> &'static str {
        match self {
            Self::Violation => "guardrail_violation",
            Self::Execution => "guardrail_error",
        }
    }

    pub(super) fn message(self) -> &'static str {
        match self {
            Self::Violation => "Response blocked by output guardrails",
            Self::Execution => "Output guardrail check failed",
        }
    }
}

#[derive(Default)]
struct SurfaceState {
    context: String,
    chars_since_check: usize,
}

pub(super) struct StreamOutputGuardrail {
    engine: Arc<GuardrailEngine>,
    active: bool,
    check_chars: usize,
    surfaces: BTreeMap<u32, SurfaceState>,
    pending: Vec<Bytes>,
    pending_bytes: usize,
    decision_sink: Option<GuardrailDecisionSink>,
}

impl StreamOutputGuardrail {
    pub(super) fn new(engine: Arc<GuardrailEngine>) -> Self {
        let config = engine.config();
        Self {
            active: engine.is_enabled() && config.check_output,
            check_chars: config.stream_output_check_chars,
            engine,
            surfaces: BTreeMap::new(),
            pending: Vec::new(),
            pending_bytes: 0,
            decision_sink: None,
        }
    }

    pub(super) fn with_decision_sink(mut self, sink: GuardrailDecisionSink) -> Self {
        self.decision_sink = Some(sink);
        self
    }

    pub(super) async fn push(
        &mut self,
        text: &str,
        event: Bytes,
    ) -> Result<Vec<Bytes>, StreamGuardrailError> {
        self.push_many(vec![(0, text.to_string())], event).await
    }

    pub(super) async fn push_many(
        &mut self,
        deltas: Vec<(u32, String)>,
        event: Bytes,
    ) -> Result<Vec<Bytes>, StreamGuardrailError> {
        if !self.active {
            return Ok(vec![event]);
        }

        if deltas.iter().all(|(_, text)| text.is_empty()) {
            if self.pending.is_empty() {
                return Ok(vec![event]);
            }
            let mut released = self.finish().await?;
            released.push(event);
            return Ok(released);
        }

        if event.len() > MAX_PENDING_BYTES {
            return Err(StreamGuardrailError::Execution);
        }
        let mut released = if self.pending_bytes.saturating_add(event.len()) > MAX_PENDING_BYTES {
            self.finish().await?
        } else {
            Vec::new()
        };
        let mut crossed_window = false;
        for (surface, text) in deltas {
            crossed_window |= self.append_and_check(surface, &text).await?;
        }
        self.pending_bytes = self.pending_bytes.saturating_add(event.len());
        self.pending.push(event);

        if crossed_window {
            released.extend(self.finish().await?);
            return Ok(released);
        }

        if !self
            .surfaces
            .values()
            .all(|state| state.chars_since_check == 0)
            && self.pending_bytes < MAX_PENDING_BYTES
        {
            return Ok(released);
        }
        if self
            .surfaces
            .values()
            .any(|state| state.chars_since_check > 0)
        {
            released.extend(self.finish().await?);
            return Ok(released);
        }
        released.extend(self.release_pending()?);
        Ok(released)
    }

    pub(super) async fn finish(&mut self) -> Result<Vec<Bytes>, StreamGuardrailError> {
        if self.active {
            let dirty = self
                .surfaces
                .iter()
                .filter_map(|(surface, state)| (state.chars_since_check > 0).then_some(*surface))
                .collect::<Vec<_>>();
            for surface in dirty {
                self.check_surface(surface).await?;
            }
        }
        self.release_pending()
    }

    pub(super) async fn push_many_until_closed(
        &mut self,
        tx: &mpsc::Sender<Bytes>,
        deltas: Vec<(u32, String)>,
        event: Bytes,
    ) -> Result<Option<Vec<Bytes>>, StreamGuardrailError> {
        tokio::select! {
            biased;
            _ = tx.closed() => Ok(None),
            result = self.push_many(deltas, event) => result.map(Some),
        }
    }

    pub(super) async fn push_until_closed(
        &mut self,
        tx: &mpsc::Sender<Bytes>,
        text: &str,
        event: Bytes,
    ) -> Result<Option<Vec<Bytes>>, StreamGuardrailError> {
        tokio::select! {
            biased;
            _ = tx.closed() => Ok(None),
            result = self.push(text, event) => result.map(Some),
        }
    }

    pub(super) async fn finish_until_closed(
        &mut self,
        tx: &mpsc::Sender<Bytes>,
    ) -> Result<Option<Vec<Bytes>>, StreamGuardrailError> {
        tokio::select! {
            biased;
            _ = tx.closed() => Ok(None),
            result = self.finish() => result.map(Some),
        }
    }

    pub(super) async fn flush_to_until_closed(
        &mut self,
        tx: &mpsc::Sender<Bytes>,
    ) -> Result<Option<usize>, StreamGuardrailError> {
        let Some(pending) = self.finish_until_closed(tx).await? else {
            return Ok(None);
        };
        let mut flushed = 0usize;
        for event in pending {
            let send_result = tokio::select! {
                biased;
                _ = tx.closed() => return Ok(None),
                sent = tx.send(event) => sent,
            };
            if send_result.is_err() {
                return Ok(None);
            }
            flushed += 1;
        }
        Ok(Some(flushed))
    }

    async fn append_and_check(
        &mut self,
        surface: u32,
        text: &str,
    ) -> Result<bool, StreamGuardrailError> {
        if !text.is_empty() && !self.surfaces.contains_key(&surface) {
            if self.surfaces.len() >= MAX_SURFACES {
                return Err(StreamGuardrailError::Execution);
            }
            self.surfaces.insert(surface, SurfaceState::default());
        }
        let mut crossed_window = false;
        let mut start = 0;
        while start < text.len() {
            let chars_since_check = self
                .surfaces
                .get(&surface)
                .ok_or(StreamGuardrailError::Execution)?
                .chars_since_check;
            let remaining = self.check_chars.saturating_sub(chars_since_check).max(1);
            let end = prefix_end(text, start, remaining);
            let should_check = {
                let state = self
                    .surfaces
                    .get_mut(&surface)
                    .ok_or(StreamGuardrailError::Execution)?;
                state.context.push_str(&text[start..end]);
                state.chars_since_check += text[start..end].chars().count();
                state.chars_since_check >= self.check_chars
            };
            start = end;
            if should_check {
                self.check_surface(surface).await?;
                crossed_window = true;
            }
        }
        Ok(crossed_window)
    }

    async fn check_surface(&mut self, surface: u32) -> Result<(), StreamGuardrailError> {
        let content = self
            .surfaces
            .get(&surface)
            .map(|state| state.context.as_str())
            .unwrap_or_default();
        let result = self
            .engine
            .check_output(content)
            .await
            .map_err(|_| StreamGuardrailError::Execution)?;
        if result.action != GuardrailAction::Allow
            && let Some(sink) = &self.decision_sink
        {
            sink.emit("output", &result);
        }
        enforce(result)?;

        if let Some(state) = self.surfaces.get_mut(&surface) {
            state.chars_since_check = 0;
            retain_tail_chars(&mut state.context, CONTEXT_OVERLAP_CHARS);
        }
        Ok(())
    }

    fn release_pending(&mut self) -> Result<Vec<Bytes>, StreamGuardrailError> {
        self.pending_bytes = 0;
        Ok(std::mem::take(&mut self.pending))
    }
}

fn retain_tail_chars(text: &mut String, max_chars: usize) {
    let count = text.chars().count();
    if count > max_chars {
        let keep_from = text
            .char_indices()
            .nth(count - max_chars)
            .map_or(text.len(), |(offset, _)| offset);
        text.drain(..keep_from);
    }
}

fn prefix_end(text: &str, start: usize, max_chars: usize) -> usize {
    text[start..]
        .char_indices()
        .nth(max_chars)
        .map_or(text.len(), |(offset, _)| start + offset)
}

fn enforce(result: CheckResult) -> Result<(), StreamGuardrailError> {
    if result.is_blocked() {
        Err(StreamGuardrailError::Violation)
    } else if result.is_modified() {
        Err(StreamGuardrailError::Execution)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::models::gateway::GatewayConfig;

    fn guardrail(check_chars: usize, enabled: bool) -> StreamOutputGuardrail {
        let mut config = GatewayConfig::default().guardrails;
        config.enabled = enabled;
        config.stream_output_check_chars = check_chars;
        let engine = GuardrailEngine::new(config).expect("test guardrails should compile");
        StreamOutputGuardrail::new(Arc::new(engine))
    }

    #[tokio::test]
    async fn safe_event_is_released_after_reaching_the_window() {
        let mut guardrail = guardrail(4, true);
        let event = Bytes::from_static(b"safe-event");

        let released = guardrail
            .push("safe", event.clone())
            .await
            .expect("safe output should pass");

        assert_eq!(released, vec![event]);
        assert!(guardrail.finish().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn event_is_released_when_a_window_is_crossed_with_a_remainder() {
        let mut guardrail = guardrail(4, true);
        let event = Bytes::from_static(b"safe-event");

        assert_eq!(
            guardrail.push("safe!", event.clone()).await.unwrap(),
            vec![event]
        );
        assert!(guardrail.finish().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn split_violation_is_blocked_before_pending_events_are_released() {
        let mut guardrail = guardrail(256, true);

        assert!(
            guardrail
                .push("System ", Bytes::from_static(b"first"))
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            guardrail
                .push("prompt: hidden policy", Bytes::from_static(b"second"))
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            guardrail.finish().await.unwrap_err(),
            StreamGuardrailError::Violation
        );
    }

    #[tokio::test]
    async fn disabled_guardrails_do_not_buffer_events() {
        let mut guardrail = guardrail(256, false);
        let event = Bytes::from_static(b"event");

        assert_eq!(
            guardrail
                .push("System prompt: hidden policy", event.clone())
                .await
                .unwrap(),
            vec![event]
        );
    }

    #[tokio::test]
    async fn non_text_event_cannot_overtake_pending_text() {
        let mut guardrail = guardrail(256, true);
        let text = Bytes::from_static(b"text");
        let terminal = Bytes::from_static(b"terminal");

        assert!(
            guardrail
                .push("safe", text.clone())
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            guardrail.push("", terminal.clone()).await.unwrap(),
            vec![text, terminal]
        );
        assert!(guardrail.finish().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn interleaved_choices_keep_independent_detection_contexts() {
        let mut guardrail = guardrail(256, true);

        assert!(
            guardrail
                .push_many(
                    vec![(0, "System ".to_string()), (1, "safe ".to_string())],
                    Bytes::from_static(b"first"),
                )
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            guardrail
                .push_many(
                    vec![(1, "text".to_string()), (0, "prompt: hidden".to_string())],
                    Bytes::from_static(b"second"),
                )
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            guardrail.finish().await.unwrap_err(),
            StreamGuardrailError::Violation
        );
    }

    #[tokio::test]
    async fn rolling_context_is_bounded_and_oversized_events_fail_closed() {
        let mut guardrail = guardrail(4096, true);
        let oversized_event = Bytes::from(vec![b'x'; MAX_PENDING_BYTES + 1]);

        assert_eq!(
            guardrail
                .push("safe", oversized_event.clone())
                .await
                .unwrap_err(),
            StreamGuardrailError::Execution
        );
        assert!(guardrail.pending.is_empty());

        guardrail
            .push(&"a".repeat(9000), Bytes::from_static(b"long"))
            .await
            .unwrap();
        guardrail.finish().await.unwrap();
        assert!(
            guardrail
                .surfaces
                .values()
                .all(|state| state.context.chars().count() <= CONTEXT_OVERLAP_CHARS)
        );
    }

    #[tokio::test]
    async fn surface_count_is_bounded() {
        let mut guardrail = guardrail(4096, true);
        let deltas = (0..=MAX_SURFACES as u32)
            .map(|surface| (surface, "x".to_string()))
            .collect();

        assert_eq!(
            guardrail
                .push_many(deltas, Bytes::from_static(b"event"))
                .await
                .unwrap_err(),
            StreamGuardrailError::Execution
        );
        assert_eq!(guardrail.surfaces.len(), MAX_SURFACES);
    }

    #[tokio::test]
    async fn closed_client_cancels_guardrail_work() {
        let mut guardrail = guardrail(1, true);
        let (tx, rx) = mpsc::channel(1);
        drop(rx);

        let result = guardrail
            .push_many_until_closed(
                &tx,
                vec![(0, "System prompt: hidden".to_string())],
                Bytes::from_static(b"event"),
            )
            .await
            .unwrap();

        assert!(result.is_none());
        assert!(guardrail.surfaces.is_empty());
    }

    #[test]
    fn result_mapping_fails_closed_for_block_and_mask() {
        assert_eq!(
            enforce(CheckResult::block(Vec::new())).unwrap_err(),
            StreamGuardrailError::Violation
        );
        assert_eq!(
            enforce(CheckResult::mask("masked".to_string(), Vec::new())).unwrap_err(),
            StreamGuardrailError::Execution
        );
        assert!(enforce(CheckResult::pass()).is_ok());
    }
}
