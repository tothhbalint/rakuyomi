use log::{debug, warn};
use wasmi::Caller;

use super::{mem::read_string, store::WasmStore};

pub fn abort(caller: Caller<'_, WasmStore>) {
    warn!("source {} requested an abort", caller.data().id);
}

pub fn print(mut caller: Caller<'_, WasmStore>, pointer: i32, length: i32) {
    match read_string(&mut caller, pointer, length) {
        Some(message) => debug!("[{}] {}", caller.data().id, message),
        None => warn!("failed to read a log message from source"),
    }
}

pub fn sleep(_caller: Caller<'_, WasmStore>, seconds: i32) {
    if seconds > 0 {
        std::thread::sleep(std::time::Duration::from_secs(seconds as u64));
    }
}

pub fn send_partial_result(mut caller: Caller<'_, WasmStore>, pointer: i32) {
    // Partial results are only useful for streaming home layouts. We don't expose
    // the home API in rakuyomi, so just consume the value to avoid leaking it.
    if let Some(bytes) = super::mem::read_encoded_buffer(&mut caller, pointer) {
        debug!(
            "source {} sent a partial result ({} bytes)",
            caller.data().id,
            bytes.len()
        );
    }
}
