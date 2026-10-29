# game_net

A generic, cross-game networking protocol abstraction and dual-mode codec in Rust.

## Features
- **Generic Protocol Envelopes**: `ClientEnvelope<TInput, TCmd>` and `ServerEnvelope<TSnapshot, TEvent>`.
- **Dual-Mode Codec (`DualCodec`)**:
  - JSON text encoding for easy debugging in browser DevTools.
  - Postcard binary encoding for production bandwidth and speed optimization.
  - Auto-adaptive decoding.
- **WebAssembly & `no_std` Friendly**.

## License
MIT OR Apache-2.0
