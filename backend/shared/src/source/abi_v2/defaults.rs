use log::warn;
use wasmi::Caller;

use super::{
    mem::{read_encoded_buffer, read_string},
    store::{DefaultValue, StoreItem, WasmStore},
};

const INVALID_KEY: i32 = -1;
const INVALID_VALUE: i32 = -2;
const FAILED_DECODING: i32 = -4;

pub fn get(mut caller: Caller<'_, WasmStore>, key_ptr: i32, key_len: i32) -> i32 {
    let Some(key) = read_string(&mut caller, key_ptr, key_len) else {
        return INVALID_KEY;
    };
    let Some(value) = caller.data().defaults.get(&key).cloned() else {
        return INVALID_VALUE;
    };

    match value {
        DefaultValue::Data(data) => caller.data_mut().store.store(StoreItem::Encoded(data)),
        DefaultValue::Null => INVALID_VALUE,
        DefaultValue::Bool(value) => caller.data_mut().store.store_encoded(&value),
        DefaultValue::Int(value) => caller.data_mut().store.store_encoded(&value),
        DefaultValue::Float(value) => caller.data_mut().store.store_encoded(&value),
        DefaultValue::String(value) => caller.data_mut().store.store_encoded(&value),
        DefaultValue::StringArray(value) => caller.data_mut().store.store_encoded(&value),
    }
}

pub fn set(
    mut caller: Caller<'_, WasmStore>,
    key_ptr: i32,
    key_len: i32,
    kind: i32,
    value_ptr: i32,
) -> i32 {
    let Some(key) = read_string(&mut caller, key_ptr, key_len) else {
        return INVALID_KEY;
    };
    let Some(data) = read_encoded_buffer(&mut caller, value_ptr) else {
        return FAILED_DECODING;
    };

    let value = match kind {
        0 => Some(DefaultValue::Data(data)),
        1 => postcard::from_bytes::<bool>(&data)
            .ok()
            .map(DefaultValue::Bool),
        2 => postcard::from_bytes::<i32>(&data)
            .ok()
            .map(DefaultValue::Int),
        3 => postcard::from_bytes::<f32>(&data)
            .ok()
            .map(DefaultValue::Float),
        4 => postcard::from_bytes::<String>(&data)
            .ok()
            .map(DefaultValue::String),
        5 => postcard::from_bytes::<Vec<String>>(&data)
            .ok()
            .map(DefaultValue::StringArray),
        6 => Some(DefaultValue::Null),
        _ => None,
    };

    let Some(value) = value else {
        warn!(
            "source {} sent an invalid default value for key {}",
            caller.data().id,
            key
        );
        return FAILED_DECODING;
    };

    caller.data_mut().defaults.insert(key, value);
    0
}
