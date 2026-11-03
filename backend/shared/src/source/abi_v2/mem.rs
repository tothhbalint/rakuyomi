use wasm_shared::get_memory;
use wasmi::Caller;

use super::store::WasmStore;

pub fn read_bytes(caller: &mut Caller<'_, WasmStore>, offset: i32, length: i32) -> Option<Vec<u8>> {
    if offset < 0 || length < 0 {
        return None;
    }
    let offset = offset as usize;
    let length = length as usize;
    let memory = get_memory(caller)?;
    let mut buffer = vec![0u8; length];
    memory.read(caller, offset, buffer.as_mut_slice()).ok()?;
    Some(buffer)
}

pub fn read_string(caller: &mut Caller<'_, WasmStore>, offset: i32, length: i32) -> Option<String> {
    let bytes = read_bytes(caller, offset, length)?;
    String::from_utf8(bytes).ok()
}

pub fn write_bytes(caller: &mut Caller<'_, WasmStore>, offset: i32, bytes: &[u8]) -> Option<()> {
    if offset < 0 {
        return None;
    }
    let memory = get_memory(caller)?;
    memory.write(caller, offset as usize, bytes).ok()
}

pub fn read_u32(caller: &mut Caller<'_, WasmStore>, offset: i32) -> Option<u32> {
    let bytes = read_bytes(caller, offset, 4)?;
    Some(u32::from_le_bytes(bytes.try_into().ok()?))
}

/// Reads a value encoded by `aidoku::imports::std::encode`: an 8 byte
/// `[length, capacity]` header followed by the actual bytes.
pub fn read_encoded_buffer(caller: &mut Caller<'_, WasmStore>, pointer: i32) -> Option<Vec<u8>> {
    let length = read_u32(caller, pointer)?;
    if length < 8 {
        return None;
    }
    read_bytes(caller, pointer + 8, (length - 8) as i32)
}
