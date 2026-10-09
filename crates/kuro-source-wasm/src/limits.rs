//! Store limits and the epoch ticker for guest execution (area 18, T-18a-006).

use std::sync::OnceLock;
use std::time::Duration;

use wasmtime::{Config, Engine, StoreLimits, StoreLimitsBuilder};

use crate::error::WasmError;

/// Largest `source.wasm` the host compiles (FEATURE-PARITY-2 X-07).
pub const MAX_WASM_BYTES: usize = 32 << 20;
/// Most linear memory one guest instance may hold (W3).
pub const MEMORY_LIMIT_BYTES: usize = 256 << 20;
/// The epoch tick. Deadlines are counted in ticks.
pub const EPOCH_TICK: Duration = Duration::from_millis(100);
/// A guest call traps after this many ticks, 30 s at [`EPOCH_TICK`] (W2).
pub const CALL_DEADLINE_TICKS: u64 = 300;

static ENGINE: OnceLock<Result<Engine, String>> = OnceLock::new();

/// The process-wide engine. The first call starts the one epoch ticker of the process.
pub fn engine() -> Result<&'static Engine, WasmError> {
    ENGINE
        .get_or_init(build_engine)
        .as_ref()
        .map_err(|e| WasmError::Engine(e.clone()))
}

fn build_engine() -> Result<Engine, String> {
    let mut config = Config::new();
    config.wasm_component_model(true);
    config.epoch_interruption(true);
    let engine = Engine::new(&config).map_err(|e| e.to_string())?;
    let ticker = engine.clone();
    std::thread::Builder::new()
        .name("kuro-epoch".to_owned())
        .spawn(move || {
            loop {
                std::thread::sleep(EPOCH_TICK);
                ticker.increment_epoch();
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(engine)
}

/// The limits of one guest store: at most [`MEMORY_LIMIT_BYTES`] of linear memory.
pub fn store_limits() -> StoreLimits {
    StoreLimitsBuilder::new().memory_size(MEMORY_LIMIT_BYTES).build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_guest_limits_match_the_plan() {
        assert_eq!(MEMORY_LIMIT_BYTES, 256 * 1024 * 1024);
        assert_eq!(MAX_WASM_BYTES, 32 * 1024 * 1024);
        assert_eq!(EPOCH_TICK.as_millis(), 100);
        assert_eq!(
            CALL_DEADLINE_TICKS, 300,
            "300 ticks of 100 ms is the 30 s call deadline"
        );
    }

    #[test]
    fn there_is_one_engine_per_process() {
        let first = engine().expect("engine");
        let second = engine().expect("engine");
        assert!(std::ptr::eq(first, second));
    }
}
