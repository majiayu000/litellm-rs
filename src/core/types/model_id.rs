//! Borrowed parsing for optionally provider-qualified model identifiers.

/// A model identifier split into its original wire form, optional provider,
/// and provider-local model name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelIdRef<'a> {
    raw: &'a str,
    provider: Option<&'a str>,
    model: &'a str,
}

impl<'a> ModelIdRef<'a> {
    /// Parse an identifier without allocating or rewriting its wire value.
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

    /// Return the provider qualifier, if present.
    pub fn provider(self) -> Option<&'a str> {
        self.provider
    }

    /// Return the provider-local portion of the identifier.
    pub fn model(self) -> &'a str {
        self.model
    }

    /// Return the provider-local model name when this identifier is either
    /// unqualified or explicitly qualified for `expected`.
    pub fn for_provider(self, expected: &str) -> Option<&'a str> {
        if self.model.is_empty() {
            return None;
        }

        match self.provider {
            None => Some(self.model),
            Some(provider) if !provider.is_empty() && provider.eq_ignore_ascii_case(expected) => {
                Some(self.model)
            }
            Some(_) => None,
        }
    }
}
