//! Lossless Anthropic thinking continuation types.
//!
//! These additive extension types keep opaque provider continuation data out of
//! the existing public `ThinkingContent` enum and chat-message structs. HTTP,
//! Responses, and SSE adapters can opt into this carrier without breaking
//! existing exhaustive matches or struct literals.

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use std::{borrow::Cow, fmt};
use thiserror::Error;

/// Invalid opaque data in an Anthropic thinking continuation.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum AnthropicContinuationError {
    /// A thinking block omitted its required verification signature.
    #[error("Anthropic thinking signature must not be empty")]
    EmptySignature,
    /// A redacted block omitted its required encrypted payload.
    #[error("Anthropic redacted thinking data must not be empty")]
    EmptyRedactedData,
}

/// An opaque, non-empty Anthropic thinking verification signature.
///
/// `Debug` and `Display` intentionally redact the value. Provider adapters that
/// must replay the signature can access it explicitly with [`Self::expose`].
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct AnthropicSignature(String);

impl AnthropicSignature {
    /// Expose the opaque signature for lossless provider forwarding.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for AnthropicSignature {
    type Error = AnthropicContinuationError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value.is_empty() {
            Err(AnthropicContinuationError::EmptySignature)
        } else {
            Ok(Self(value.to_string()))
        }
    }
}

impl TryFrom<String> for AnthropicSignature {
    type Error = AnthropicContinuationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() {
            Err(AnthropicContinuationError::EmptySignature)
        } else {
            Ok(Self(value))
        }
    }
}

impl Serialize for AnthropicSignature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.expose())
    }
}

impl<'de> Deserialize<'de> for AnthropicSignature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_from(value).map_err(D::Error::custom)
    }
}

impl fmt::Debug for AnthropicSignature {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "AnthropicSignature([redacted; {} bytes])",
            self.0.len()
        )
    }
}

impl fmt::Display for AnthropicSignature {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "[redacted; {} bytes]", self.0.len())
    }
}

/// An opaque, non-empty Anthropic safety-redacted thinking payload.
///
/// `Debug` and `Display` intentionally redact the value. Provider adapters that
/// must replay the payload can access it explicitly with [`Self::expose`].
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct AnthropicRedactedData(String);

impl AnthropicRedactedData {
    /// Expose the opaque payload for lossless provider forwarding.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for AnthropicRedactedData {
    type Error = AnthropicContinuationError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value.is_empty() {
            Err(AnthropicContinuationError::EmptyRedactedData)
        } else {
            Ok(Self(value.to_string()))
        }
    }
}

impl TryFrom<String> for AnthropicRedactedData {
    type Error = AnthropicContinuationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() {
            Err(AnthropicContinuationError::EmptyRedactedData)
        } else {
            Ok(Self(value))
        }
    }
}

impl Serialize for AnthropicRedactedData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.expose())
    }
}

impl<'de> Deserialize<'de> for AnthropicRedactedData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_from(value).map_err(D::Error::custom)
    }
}

impl fmt::Debug for AnthropicRedactedData {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "AnthropicRedactedData([redacted; {} bytes])",
            self.0.len()
        )
    }
}

impl fmt::Display for AnthropicRedactedData {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "[redacted; {} bytes]", self.0.len())
    }
}

/// A lossless Anthropic Messages API thinking continuation block.
///
/// This is a new extension type rather than a variant on the existing public
/// `ThinkingContent` enum, preserving exhaustive-match source compatibility.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum AnthropicThinkingBlock {
    /// Visible or omitted thinking with its required verification signature.
    Thinking {
        /// Summarized thinking text, empty when display is omitted.
        thinking: String,
        /// Opaque signature required for continuation replay.
        signature: AnthropicSignature,
    },
    /// Safety-redacted thinking with its required opaque encrypted payload.
    RedactedThinking {
        /// Opaque data required for continuation replay.
        data: AnthropicRedactedData,
    },
}

/// Ordered, lossless Anthropic thinking continuation blocks.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct AnthropicThinkingContent(Vec<AnthropicThinkingBlock>);

impl AnthropicThinkingContent {
    /// Create an ordered continuation from validated blocks.
    pub fn new(blocks: Vec<AnthropicThinkingBlock>) -> Self {
        Self(blocks)
    }

    /// Return every block in upstream order.
    pub fn blocks(&self) -> &[AnthropicThinkingBlock] {
        &self.0
    }

    /// Consume the continuation and return every block in upstream order.
    pub fn into_blocks(self) -> Vec<AnthropicThinkingBlock> {
        self.0
    }

    /// Return all non-empty visible thinking text in upstream order.
    ///
    /// A single visible block is borrowed. Multiple visible blocks are joined
    /// without separators because upstream block boundaries already determine
    /// the exact text fragments.
    pub fn as_text(&self) -> Option<Cow<'_, str>> {
        let mut visible = self.0.iter().filter_map(|block| match block {
            AnthropicThinkingBlock::Thinking { thinking, .. } if !thinking.is_empty() => {
                Some(thinking.as_str())
            }
            _ => None,
        });
        let first = visible.next()?;
        let Some(second) = visible.next() else {
            return Some(Cow::Borrowed(first));
        };

        let mut joined = String::with_capacity(first.len() + second.len());
        joined.push_str(first);
        joined.push_str(second);
        for text in visible {
            joined.push_str(text);
        }
        Some(Cow::Owned(joined))
    }

    /// Whether the continuation contains at least one redacted block.
    pub fn has_redacted_block(&self) -> bool {
        self.0
            .iter()
            .any(|block| matches!(block, AnthropicThinkingBlock::RedactedThinking { .. }))
    }
}

/// Private ordering metadata used to replay thinking and tool-use blocks in
/// the same relative order returned by Anthropic.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum AnthropicContentBlockOrder {
    Thinking { index: usize },
    ToolUse { index: usize },
}

/// Additive, provider-explicit continuation extensions for a chat message.
///
/// Fields are private so this carrier can grow without breaking external struct
/// literals. Callers opt in through constructors and accessors.
#[non_exhaustive]
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ChatMessageExtensions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    anthropic_thinking: Option<AnthropicThinkingContent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    anthropic_block_order: Option<Vec<AnthropicContentBlockOrder>>,
}

impl ChatMessageExtensions {
    /// Create an empty extension carrier.
    pub fn new() -> Self {
        Self::default()
    }

    /// Attach an Anthropic thinking continuation.
    pub fn with_anthropic_thinking(mut self, content: AnthropicThinkingContent) -> Self {
        self.anthropic_thinking = Some(content);
        self
    }

    /// Return the Anthropic thinking continuation, if present.
    pub fn anthropic_thinking(&self) -> Option<&AnthropicThinkingContent> {
        self.anthropic_thinking.as_ref()
    }

    pub(crate) fn with_anthropic_block_order(
        mut self,
        order: Vec<AnthropicContentBlockOrder>,
    ) -> Self {
        self.anthropic_block_order = Some(order);
        self
    }

    pub(crate) fn anthropic_block_order(&self) -> Option<&[AnthropicContentBlockOrder]> {
        self.anthropic_block_order.as_deref()
    }

    /// Whether this carrier has no provider extensions.
    pub fn is_empty(&self) -> bool {
        self.anthropic_thinking
            .as_ref()
            .is_none_or(|thinking| thinking.blocks().is_empty())
    }
}
