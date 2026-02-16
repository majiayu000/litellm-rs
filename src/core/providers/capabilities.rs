//! Compile-time provider capability verification
//!
//! This module uses Rust's type system to enforce provider capabilities
//! at compile time, preventing runtime errors from calling unsupported methods.

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
#[allow(dead_code)]
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    struct MockProvider;

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
