use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

use super::ProviderError;
use crate::core::types::anthropic_continuation::{AnthropicThinkingContent, ChatMessageExtensions};
use crate::core::types::{chat::ChatRequest, responses::ChatResponse};

/// Payload-free ordering metadata for replaying Anthropic response blocks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum AnthropicContentBlockOrder {
    Thinking { index: usize },
    Text { start: usize, end: usize },
    Refusal { start: usize, end: usize },
    ToolUse { index: usize },
}

/// Private per-message continuation state used between HTTP and providers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ChatMessageContinuation {
    extensions: ChatMessageExtensions,
    anthropic_block_order: Option<Vec<AnthropicContentBlockOrder>>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChatMessageContinuationWire {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    anthropic_thinking: Option<AnthropicThinkingContent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    anthropic_block_order: Option<Vec<AnthropicContentBlockOrder>>,
}

impl ChatMessageContinuation {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn with_anthropic_thinking(mut self, thinking: AnthropicThinkingContent) -> Self {
        self.extensions = self.extensions.with_anthropic_thinking(thinking);
        self
    }

    pub(crate) fn with_anthropic_block_order(
        mut self,
        order: Vec<AnthropicContentBlockOrder>,
    ) -> Self {
        self.anthropic_block_order = Some(order);
        self
    }

    pub(crate) fn without_anthropic_block_order(mut self) -> Self {
        self.anthropic_block_order = None;
        self
    }

    pub(crate) fn anthropic_thinking(&self) -> Option<&AnthropicThinkingContent> {
        self.extensions.anthropic_thinking()
    }

    pub(crate) fn anthropic_block_order(&self) -> Option<&[AnthropicContentBlockOrder]> {
        self.anthropic_block_order.as_deref()
    }

    pub(crate) fn has_visible_thinking(&self) -> bool {
        self.anthropic_thinking()
            .and_then(|thinking| thinking.as_text())
            .is_some_and(|text| !text.is_empty())
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.anthropic_thinking()
            .is_none_or(|thinking| thinking.blocks().is_empty())
            && self.anthropic_block_order.is_none()
    }

    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        if self.anthropic_block_order.is_some()
            && self
                .anthropic_thinking()
                .is_none_or(|thinking| thinking.blocks().is_empty())
        {
            return Err(
                "Anthropic block order requires a non-empty Anthropic thinking continuation",
            );
        }
        Ok(())
    }
}

impl From<ChatMessageExtensions> for ChatMessageContinuation {
    fn from(extensions: ChatMessageExtensions) -> Self {
        Self {
            extensions,
            anthropic_block_order: None,
        }
    }
}

impl Serialize for ChatMessageContinuation {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ChatMessageContinuationWire {
            anthropic_thinking: self.anthropic_thinking().cloned(),
            anthropic_block_order: self.anthropic_block_order.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ChatMessageContinuation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ChatMessageContinuationWire::deserialize(deserializer)?;
        let mut continuation = Self::new();
        if let Some(thinking) = wire.anthropic_thinking {
            continuation = continuation.with_anthropic_thinking(thinking);
        }
        continuation.anthropic_block_order = wire.anthropic_block_order;
        continuation.validate().map_err(D::Error::custom)?;
        Ok(continuation)
    }
}

/// Internal synchronous request plus one continuation sidecar per message.
#[derive(Debug, Clone)]
pub(crate) struct ChatContinuationRequest {
    request: ChatRequest,
    message_continuations: Vec<ChatMessageContinuation>,
}

impl ChatContinuationRequest {
    pub(crate) fn new(
        request: ChatRequest,
        message_continuations: Vec<ChatMessageContinuation>,
    ) -> Result<Self, ProviderError> {
        if request.messages.len() != message_continuations.len() {
            return Err(ProviderError::invalid_request(
                "continuation",
                format!(
                    "message continuation length mismatch: expected {}, got {}",
                    request.messages.len(),
                    message_continuations.len()
                ),
            ));
        }
        Ok(Self {
            request,
            message_continuations,
        })
    }

    pub(crate) fn request(&self) -> &ChatRequest {
        &self.request
    }

    pub(crate) fn has_continuation(&self) -> bool {
        self.message_continuations
            .iter()
            .any(|item| !item.is_empty())
    }

    pub(crate) fn into_parts(self) -> (ChatRequest, Vec<ChatMessageContinuation>) {
        (self.request, self.message_continuations)
    }
}

/// Internal synchronous response plus one continuation sidecar per choice.
#[derive(Debug, Clone)]
pub(crate) struct ChatContinuationResponse {
    response: ChatResponse,
    choice_continuations: Vec<ChatMessageContinuation>,
}

impl ChatContinuationResponse {
    pub(crate) fn new(
        response: ChatResponse,
        choice_continuations: Vec<ChatMessageContinuation>,
    ) -> Result<Self, ProviderError> {
        if response.choices.len() != choice_continuations.len() {
            return Err(ProviderError::invalid_request(
                "continuation",
                format!(
                    "choice continuation length mismatch: expected {}, got {}",
                    response.choices.len(),
                    choice_continuations.len()
                ),
            ));
        }
        Ok(Self {
            response,
            choice_continuations,
        })
    }

    pub(crate) fn response(&self) -> &ChatResponse {
        &self.response
    }

    #[cfg(test)]
    pub(crate) fn choice_continuations(&self) -> &[ChatMessageContinuation] {
        &self.choice_continuations
    }

    pub(crate) fn into_parts(self) -> (ChatResponse, Vec<ChatMessageContinuation>) {
        (self.response, self.choice_continuations)
    }
}
