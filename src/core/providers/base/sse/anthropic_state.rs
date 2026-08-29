use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::anthropic_continuation::{
    AnthropicRedactedData, AnthropicSignature, AnthropicThinkingBlock,
};

enum ActiveThinkingBlock {
    Thinking { thinking: String, signature: String },
    Redacted { data: AnthropicRedactedData },
}

#[derive(Clone, Copy)]
pub(super) enum ActiveContentBlock {
    Text,
    ToolUse,
    Ignored,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DeltaDisposition {
    Emit,
    Ignore,
}

impl ActiveContentBlock {
    fn kind(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::ToolUse => "tool_use",
            Self::Ignored => "future",
        }
    }

    fn accepts(self, delta_type: &str) -> bool {
        match self {
            Self::Text => matches!(delta_type, "text_delta" | "citations_delta"),
            Self::ToolUse => delta_type == "input_json_delta",
            Self::Ignored => true,
        }
    }
}

impl ActiveThinkingBlock {
    fn kind(&self) -> &'static str {
        match self {
            Self::Thinking { .. } => "thinking",
            Self::Redacted { .. } => "redacted_thinking",
        }
    }
}

#[derive(Default)]
pub(super) struct AnthropicThinkingStreamState {
    active: BTreeMap<u32, ActiveThinkingBlock>,
    active_content: BTreeMap<u32, ActiveContentBlock>,
    pub(super) completed: Vec<(u32, AnthropicThinkingBlock)>,
    completed_indexes: BTreeSet<u32>,
}

impl fmt::Debug for AnthropicThinkingStreamState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AnthropicThinkingStreamState")
            .field(
                "active_thinking_indexes",
                &self.active.keys().collect::<Vec<_>>(),
            )
            .field(
                "active_content_indexes",
                &self.active_content.keys().collect::<Vec<_>>(),
            )
            .field("completed_count", &self.completed.len())
            .finish()
    }
}

impl AnthropicThinkingStreamState {
    pub(super) fn begin_message(&mut self) -> Result<(), ProviderError> {
        self.ensure_complete("message_start")?;
        self.active_content.clear();
        self.completed.clear();
        self.completed_indexes.clear();
        Ok(())
    }

    pub(super) fn begin_content(
        &mut self,
        index: u32,
        block: ActiveContentBlock,
    ) -> Result<(), ProviderError> {
        self.ensure_index_available(index)?;
        self.active_content.insert(index, block);
        Ok(())
    }

    pub(super) fn begin_thinking(
        &mut self,
        index: u32,
        thinking: &str,
        signature: &str,
    ) -> Result<(), ProviderError> {
        self.insert(
            index,
            ActiveThinkingBlock::Thinking {
                thinking: thinking.to_string(),
                signature: signature.to_string(),
            },
        )
    }

    pub(super) fn begin_redacted(&mut self, index: u32, data: &str) -> Result<(), ProviderError> {
        let data = AnthropicRedactedData::try_from(data).map_err(|_| {
            lifecycle_error(
                index,
                format!("redacted_thinking block at index {index} has empty data"),
            )
        })?;
        self.insert(index, ActiveThinkingBlock::Redacted { data })
    }

    pub(super) fn append_thinking(
        &mut self,
        index: u32,
        fragment: &str,
    ) -> Result<(), ProviderError> {
        match self.active.get_mut(&index) {
            Some(ActiveThinkingBlock::Thinking {
                thinking,
                signature,
            }) if signature.is_empty() => {
                thinking.push_str(fragment);
                Ok(())
            }
            Some(ActiveThinkingBlock::Thinking { .. }) => Err(lifecycle_error(
                index,
                format!("thinking delta for block at index {index} arrived after its signature"),
            )),
            Some(block) => Err(lifecycle_error(
                index,
                format!("thinking delta for {} block at index {index}", block.kind()),
            )),
            None => Err(lifecycle_error(
                index,
                format!("thinking delta for inactive block at index {index}"),
            )),
        }
    }

    pub(super) fn append_signature(
        &mut self,
        index: u32,
        fragment: &str,
    ) -> Result<(), ProviderError> {
        if fragment.is_empty() {
            return Err(lifecycle_error(
                index,
                format!("thinking block at index {index} received an empty signature"),
            ));
        }
        match self.active.get_mut(&index) {
            Some(ActiveThinkingBlock::Thinking { signature, .. }) => {
                signature.push_str(fragment);
                Ok(())
            }
            Some(block) => Err(lifecycle_error(
                index,
                format!(
                    "signature delta for {} block at index {index}",
                    block.kind()
                ),
            )),
            None => Err(lifecycle_error(
                index,
                format!("signature delta for inactive block at index {index}"),
            )),
        }
    }

    pub(super) fn complete(&mut self, index: u32) -> Result<bool, ProviderError> {
        let Some(active) = self.active.get(&index) else {
            if self.active_content.remove(&index).is_some() {
                self.completed_indexes.insert(index);
                return Ok(false);
            }
            let status = if self.completed_indexes.contains(&index) {
                "completed"
            } else {
                "inactive"
            };
            return Err(lifecycle_error(
                index,
                format!("content_block_stop for {status} index {index}"),
            ));
        };
        let completed = match active {
            ActiveThinkingBlock::Thinking {
                thinking,
                signature,
            } => AnthropicThinkingBlock::Thinking {
                thinking: thinking.clone(),
                signature: AnthropicSignature::try_from(signature.as_str()).map_err(|_| {
                    lifecycle_error(
                        index,
                        format!("thinking block at index {index} is missing its signature"),
                    )
                })?,
            },
            ActiveThinkingBlock::Redacted { data } => {
                AnthropicThinkingBlock::RedactedThinking { data: data.clone() }
            }
        };
        self.active.remove(&index);
        self.completed.push((index, completed));
        self.completed_indexes.insert(index);
        Ok(true)
    }

    fn insert(&mut self, index: u32, block: ActiveThinkingBlock) -> Result<(), ProviderError> {
        self.ensure_index_available(index)?;
        self.active.insert(index, block);
        Ok(())
    }

    fn ensure_index_available(&self, index: u32) -> Result<(), ProviderError> {
        if let Some(existing) = self.active.get(&index) {
            return Err(lifecycle_error(
                index,
                format!(
                    "duplicate {} block at active index {index}",
                    existing.kind()
                ),
            ));
        }
        if let Some(existing) = self.active_content.get(&index) {
            return Err(lifecycle_error(
                index,
                format!(
                    "duplicate {} block at active index {index}",
                    existing.kind()
                ),
            ));
        }
        if self.completed_indexes.contains(&index) {
            return Err(lifecycle_error(
                index,
                format!("content block index {index} was already completed in this message"),
            ));
        }
        Ok(())
    }

    pub(super) fn validate_delta_kind(
        &self,
        index: u32,
        delta_type: &str,
    ) -> Result<DeltaDisposition, ProviderError> {
        if let Some(block) = self.active.get(&index) {
            if matches!(block, ActiveThinkingBlock::Thinking { .. })
                && matches!(delta_type, "thinking_delta" | "signature_delta")
            {
                return Ok(DeltaDisposition::Emit);
            }
            return Err(lifecycle_error(
                index,
                format!(
                    "{delta_type} delta is invalid for {} block at index {index}",
                    block.kind()
                ),
            ));
        }
        if let Some(block) = self.active_content.get(&index) {
            if block.accepts(delta_type) {
                return Ok(if matches!(block, ActiveContentBlock::Ignored) {
                    DeltaDisposition::Ignore
                } else {
                    DeltaDisposition::Emit
                });
            }
            return Err(lifecycle_error(
                index,
                format!(
                    "{delta_type} delta is invalid for {} block at index {index}",
                    block.kind()
                ),
            ));
        }
        if self.completed_indexes.contains(&index) {
            return Err(lifecycle_error(
                index,
                format!(
                    "{delta_type} delta for content block index {index} arrived after it completed"
                ),
            ));
        }
        Ok(DeltaDisposition::Emit)
    }

    pub(super) fn ensure_complete(&self, boundary: &str) -> Result<(), ProviderError> {
        let Some((index, block)) = self.active.first_key_value() else {
            return Ok(());
        };
        let detail = match block {
            ActiveThinkingBlock::Thinking { signature, .. } if signature.is_empty() => {
                "missing its signature"
            }
            _ => "missing content_block_stop",
        };
        Err(lifecycle_error(
            *index,
            format!(
                "{} block at index {index} is incomplete at {boundary}: {detail}",
                block.kind()
            ),
        ))
    }
}

fn lifecycle_error(index: u32, message: impl Into<String>) -> ProviderError {
    ProviderError::streaming_error(
        "anthropic",
        "chat.thinking",
        Some(u64::from(index)),
        None,
        message,
    )
}
