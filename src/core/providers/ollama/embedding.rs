use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::responses::{EmbeddingData, EmbeddingResponse, Usage};

pub(super) fn parse_embedding_response(
    response: &serde_json::Value,
    expected_embedding_count: usize,
    model: &str,
) -> Result<EmbeddingResponse, ProviderError> {
    let embeddings = response
        .get("embeddings")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            ProviderError::response_parsing("ollama", "Missing embeddings in response")
        })?;

    if embeddings.len() != expected_embedding_count {
        return Err(ProviderError::response_parsing(
            "ollama",
            format!(
                "Ollama returned {} embeddings for {expected_embedding_count} inputs",
                embeddings.len()
            ),
        ));
    }

    let mut expected_dimension = None;
    let mut data = Vec::with_capacity(embeddings.len());
    for (i, emb) in embeddings.iter().enumerate() {
        let values = emb.as_array().ok_or_else(|| {
            ProviderError::response_parsing(
                "ollama",
                format!("Ollama embedding at index {i} is not an array"),
            )
        })?;
        if values.is_empty() {
            return Err(ProviderError::response_parsing(
                "ollama",
                format!("Ollama embedding at index {i} is empty"),
            ));
        }
        if expected_dimension.is_some_and(|dimension| dimension != values.len()) {
            return Err(ProviderError::response_parsing(
                "ollama",
                format!("Ollama embedding at index {i} has an inconsistent dimension"),
            ));
        }
        expected_dimension.get_or_insert(values.len());
        let embedding = values
            .iter()
            .enumerate()
            .map(|(coordinate, value)| {
                value
                    .as_f64()
                    .filter(|value| value.is_finite() && (*value as f32).is_finite())
                    .map(|value| value as f32)
                    .ok_or_else(|| {
                        ProviderError::response_parsing(
                            "ollama",
                            format!(
                                "Ollama embedding at index {i} has an invalid coordinate at {coordinate}"
                            ),
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let index = u32::try_from(i).map_err(|_| {
            ProviderError::response_parsing(
                "ollama",
                "Ollama embedding response contains too many rows",
            )
        })?;

        data.push(EmbeddingData {
            object: "embedding".to_string(),
            embedding,
            index,
        });
    }

    let prompt_tokens = match response.get("prompt_eval_count") {
        None => 0,
        Some(count) => count
            .as_u64()
            .and_then(|count| u32::try_from(count).ok())
            .ok_or_else(|| {
                ProviderError::response_parsing(
                    "ollama",
                    "Ollama embedding response has an invalid prompt_eval_count",
                )
            })?,
    };

    Ok(EmbeddingResponse {
        object: "list".to_string(),
        data,
        model: format!("ollama/{model}"),
        usage: Some(Usage {
            prompt_tokens,
            completion_tokens: 0,
            total_tokens: prompt_tokens,
            prompt_tokens_details: None,
            completion_tokens_details: None,
            thinking_usage: None,
        }),
        embeddings: None,
    })
}
