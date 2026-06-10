//! Audio endpoints (transcription, translation, speech)

mod speech;
mod transcriptions;
mod translations;
mod upload;

// Re-export the public functions
pub use speech::audio_speech;
pub use transcriptions::audio_transcriptions;
pub use translations::audio_translations;
