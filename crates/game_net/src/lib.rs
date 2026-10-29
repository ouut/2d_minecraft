//! Generic Layer 0 network protocol envelopes and dual-mode codec.
//!
//! Provides protocol envelopes (`ClientEnvelope`, `ServerEnvelope`) and the `DualCodec`
//! supporting both human-readable JSON (development/debugging) and compact Postcard binary (production)
//! across games.

pub mod codec;
pub mod envelope;

pub use codec::{CodecError, CodecFormat, DualCodec};
pub use envelope::{ClientEnvelope, PlayerId, ServerEnvelope};
