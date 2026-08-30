//! Borrowed, lossless syntax views over model identifiers.

/// A model identifier split at its first `/`, without assigning semantics to
/// either segment.
///
/// The first segment is exposed as `provider` for callers that use qualified
/// IDs, but it is not validated or normalized. Some provider-native IDs, such
/// as `BAAI/bge-m3`, also contain `/`; consumers must use their own context to
/// decide whether the first segment is a provider qualifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModelIdRef<'a> {
    raw: &'a str,
    provider: Option<&'a str>,
    model: &'a str,
}

impl<'a> ModelIdRef<'a> {
    /// Split at the first `/` without allocating, validating, or rewriting.
    pub fn parse(raw: &'a str) -> Self {
        let (provider, model) = raw
            .split_once('/')
            .map_or((None, raw), |(provider, model)| (Some(provider), model));

        Self {
            raw,
            provider,
            model,
        }
    }

    /// Return the exact input supplied by the caller.
    pub fn raw(self) -> &'a str {
        self.raw
    }

    /// Return the text before the first `/`, if a slash is present.
    ///
    /// This is a syntactic view only and may be empty or provider-native data.
    pub fn provider(self) -> Option<&'a str> {
        self.provider
    }

    /// Return the complete text after the first `/`, or the full unqualified
    /// identifier when no slash is present.
    pub fn model(self) -> &'a str {
        self.model
    }
}
