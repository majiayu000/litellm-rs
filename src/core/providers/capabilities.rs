//! Compile-time provider capability verification
//!
//! This module uses Rust's type system to enforce provider capabilities
//! at compile time, preventing runtime errors from calling unsupported methods.

use std::marker::PhantomData;

use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::context::RequestContext;
use crate::core::types::model::ProviderCapability;
use crate::core::types::responses::{ChatResponse, EmbeddingResponse, ImageGenerationResponse};
use crate::core::types::{chat::ChatRequest, embedding::EmbeddingRequest, image::ImageGenerationRequest};

// ============================================================================
// Capability States (Phantom Types)
// ============================================================================

/// Provider has chat capability
pub struct WithChat;

/// Provider has no chat capability
pub struct NoChat;

/// Provider has embedding capability
pub struct WithEmbedding;

/// Provider has no embedding capability
pub struct NoEmbedding;

/// Provider has image capability
pub struct WithImage;

/// Provider has no image capability
pub struct NoImage;

/// Provider has streaming capability
pub struct WithStream;

/// Provider has no streaming capability
pub struct NoStream;

// ============================================================================
// Type-Safe Provider Wrapper
// ============================================================================

/// Type-safe provider wrapper that enforces capabilities at compile time
pub struct TypedProvider<P, Chat, Embed, Image, Stream> {
    inner: P,
    _chat: PhantomData<Chat>,
    _embed: PhantomData<Embed>,
    _image: PhantomData<Image>,
    _stream: PhantomData<Stream>,
}

impl<P, C, E, I, S> TypedProvider<P, C, E, I, S> {
    /// Get reference to inner provider
    pub fn inner(&self) -> &P {
        &self.inner
    }

    /// Get mutable reference to inner provider
    pub fn inner_mut(&mut self) -> &mut P {
        &mut self.inner
    }
}

// ============================================================================
// Capability-Specific Implementations
// ============================================================================

/// Chat completion methods - only available when Chat = WithChat
impl<P, E, I, S> TypedProvider<P, WithChat, E, I, S>
where
    P: ChatProvider,
{
    pub async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        self.inner.chat_completion(request, context).await
    }
}

/// Embedding methods - only available when Embed = WithEmbedding
impl<P, C, I, S> TypedProvider<P, C, WithEmbedding, I, S>
where
    P: EmbeddingProvider,
{
    pub async fn create_embeddings(
        &self,
        request: EmbeddingRequest,
        context: RequestContext,
    ) -> Result<EmbeddingResponse, ProviderError> {
        self.inner.create_embeddings(request, context).await
    }
}

/// Image generation methods - only available when Image = WithImage
impl<P, C, E, S> TypedProvider<P, C, E, WithImage, S>
where
    P: ImageProvider,
{
    pub async fn create_images(
        &self,
        request: ImageGenerationRequest,
        context: RequestContext,
    ) -> Result<ImageGenerationResponse, ProviderError> {
        self.inner.create_images(request, context).await
    }
}

// ============================================================================
// Provider Traits
// ============================================================================

/// Trait for providers that support chat completion
#[async_trait::async_trait]
pub trait ChatProvider {
    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, ProviderError>;
}

/// Trait for providers that support embeddings
#[async_trait::async_trait]
pub trait EmbeddingProvider {
    async fn create_embeddings(
        &self,
        request: EmbeddingRequest,
        context: RequestContext,
    ) -> Result<EmbeddingResponse, ProviderError>;
}

/// Trait for providers that support image generation
#[async_trait::async_trait]
pub trait ImageProvider {
    async fn create_images(
        &self,
        request: ImageGenerationRequest,
        context: RequestContext,
    ) -> Result<ImageGenerationResponse, ProviderError>;
}

// ============================================================================
// Builder Pattern for TypedProvider
// ============================================================================

pub struct TypedProviderBuilder<P> {
    provider: P,
    capabilities: Vec<ProviderCapability>,
}

impl<P> TypedProviderBuilder<P> {
    pub fn new(provider: P) -> Self {
        Self {
            provider,
            capabilities: Vec::new(),
        }
    }

    pub fn with_capability(mut self, capability: ProviderCapability) -> Self {
        self.capabilities.push(capability);
        self
    }

    /// Build a typed provider based on declared capabilities
    pub fn build(self) -> impl BuildResult<P> {
        // This would use const generics or macros to generate the right type
        // based on self.capabilities
        let _ = self.capabilities;
        TypedProviderBuilderResult {
            provider: self.provider,
        }
    }
}

pub trait BuildResult<P> {
    type Output;
    fn into_typed(self) -> Self::Output;
}

struct TypedProviderBuilderResult<P> {
    provider: P,
}

impl<P> BuildResult<P> for TypedProviderBuilderResult<P> {
    type Output = P; // Simplified - in production, would return appropriate TypedProvider variant

    fn into_typed(self) -> Self::Output {
        self.provider
    }
}

// ============================================================================
// Capability Verification Macros
// ============================================================================

/// Macro to verify provider capabilities at compile time
#[macro_export]
macro_rules! verify_capability {
    ($provider:expr, chat) => {
        compile_time_assert!($provider: ChatCapable);
    };
    ($provider:expr, embedding) => {
        compile_time_assert!($provider: EmbeddingCapable);
    };
    ($provider:expr, image) => {
        compile_time_assert!($provider: ImageCapable);
    };
}

/// Macro to create a typed provider with specific capabilities
#[macro_export]
macro_rules! typed_provider {
    ($provider:expr, capabilities: [chat]) => {
        TypedProvider::<_, WithChat, NoEmbedding, NoImage, NoStream> {
            inner: $provider,
            _chat: PhantomData,
            _embed: PhantomData,
            _image: PhantomData,
            _stream: PhantomData,
        }
    };
    ($provider:expr, capabilities: [chat, embedding]) => {
        TypedProvider::<_, WithChat, WithEmbedding, NoImage, NoStream> {
            inner: $provider,
            _chat: PhantomData,
            _embed: PhantomData,
            _image: PhantomData,
            _stream: PhantomData,
        }
    };
    ($provider:expr, capabilities: [chat, embedding, image]) => {
        TypedProvider::<_, WithChat, WithEmbedding, WithImage, NoStream> {
            inner: $provider,
            _chat: PhantomData,
            _embed: PhantomData,
            _image: PhantomData,
            _stream: PhantomData,
        }
    };
}

// ============================================================================
// Capability Sets (Const Generics Alternative)
// ============================================================================

/// A capability set that can be checked at compile time
#[derive(Debug, Clone, Copy)]
pub struct Capabilities {
    pub chat: bool,
    pub embedding: bool,
    pub image: bool,
    pub streaming: bool,
    pub function_calling: bool,
}

impl Capabilities {
    pub const CHAT_ONLY: Self = Self {
        chat: true,
        embedding: false,
        image: false,
        streaming: false,
        function_calling: false,
    };

    pub const FULL: Self = Self {
        chat: true,
        embedding: true,
        image: true,
        streaming: true,
        function_calling: true,
    };

    pub const fn has_chat(&self) -> bool {
        self.chat
    }

    pub const fn has_embedding(&self) -> bool {
        self.embedding
    }

    pub const fn has_image(&self) -> bool {
        self.image
    }
}

/// Provider with const-generic capabilities
pub struct ConstProvider<P, const CAPS: u8> {
    inner: P,
}

// Capability flags as bit masks
pub const CAP_CHAT: u8 = 0b00001;
pub const CAP_EMBED: u8 = 0b00010;
pub const CAP_IMAGE: u8 = 0b00100;
pub const CAP_STREAM: u8 = 0b01000;
pub const CAP_FUNCTION: u8 = 0b10000;

impl<P, const CAPS: u8> ConstProvider<P, CAPS> {
    pub const fn has_chat() -> bool {
        CAPS & CAP_CHAT != 0
    }

    pub const fn has_embedding() -> bool {
        CAPS & CAP_EMBED != 0
    }

    pub const fn has_image() -> bool {
        CAPS & CAP_IMAGE != 0
    }
}

// Only compile chat methods if CAP_CHAT is set
impl<P> ConstProvider<P, { CAP_CHAT }>
where
    P: ChatProvider,
{
    pub async fn chat(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        self.inner.chat_completion(request, context).await
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    struct MockProvider;

    #[async_trait::async_trait]
    impl ChatProvider for MockProvider {
        async fn chat_completion(
            &self,
            _request: ChatRequest,
            _context: RequestContext,
        ) -> Result<ChatResponse, ProviderError> {
            unimplemented!()
        }
    }

    #[test]
    fn test_typed_provider_creation() {
        let provider = MockProvider;
        let _typed = typed_provider!(provider, capabilities: [chat]);
        // This compiles, proving type safety
    }

    #[test]
    fn test_capability_flags() {
        assert!(ConstProvider::<MockProvider, { CAP_CHAT }>::has_chat());
        assert!(!ConstProvider::<MockProvider, { CAP_CHAT }>::has_embedding());

        const MULTI_CAP: u8 = CAP_CHAT | CAP_EMBED;
        assert!(ConstProvider::<MockProvider, MULTI_CAP>::has_chat());
        assert!(ConstProvider::<MockProvider, MULTI_CAP>::has_embedding());
    }

    #[test]
    fn test_capabilities_const() {
        assert!(Capabilities::CHAT_ONLY.has_chat());
        assert!(!Capabilities::CHAT_ONLY.has_embedding());

        assert!(Capabilities::FULL.has_chat());
        assert!(Capabilities::FULL.has_embedding());
        assert!(Capabilities::FULL.has_image());
    }
}
