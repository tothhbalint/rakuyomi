use anyhow::{Context, Result};
use chrono::{Local, NaiveDateTime, Utc};
use wasmi::Caller;

use super::{mem::read_string, store::WasmStore};

pub fn abort(caller: Caller<'_, WasmStore>) {
    super::env::abort(caller);
}

pub fn print(caller: Caller<'_, WasmStore>, pointer: i32, length: i32) {
    super::env::print(caller, pointer, length);
}

pub fn destroy(mut caller: Caller<'_, WasmStore>, rid: i32) {
    caller.data_mut().store.remove(rid);
}

pub fn buffer_len(caller: Caller<'_, WasmStore>, rid: i32) -> i32 {
    caller
        .data()
        .store
        .get(rid)
        .and_then(|item| item.as_bytes())
        .map(|bytes| bytes.len() as i32)
        .unwrap_or(-1)
}

pub fn read_buffer(mut caller: Caller<'_, WasmStore>, rid: i32, pointer: i32, size: i32) -> i32 {
    let Some(data) = caller
        .data()
        .store
        .get(rid)
        .and_then(|item| item.as_bytes())
        .map(|bytes| bytes.to_vec())
    else {
        return -1;
    };

    if size < 0 || size as usize > data.len() {
        return -3;
    }

    match super::mem::write_bytes(&mut caller, pointer, &data[..size as usize]) {
        Some(()) => 0,
        None => -3,
    }
}

pub fn current_date() -> f64 {
    Utc::now().timestamp() as f64
}

pub fn utc_offset() -> i64 {
    Local::now().offset().utc_minus_local() as i64
}

pub fn parse_date(
    mut caller: Caller<'_, WasmStore>,
    date_ptr: i32,
    date_len: i32,
    format_ptr: i32,
    format_len: i32,
    _locale_ptr: i32,
    _locale_len: i32,
    timezone_ptr: i32,
    timezone_len: i32,
) -> f64 {
    let result = parse_date_inner(
        &mut caller,
        date_ptr,
        date_len,
        format_ptr,
        format_len,
        timezone_ptr,
        timezone_len,
    );

    match result {
        Ok(timestamp) => timestamp,
        Err(_) => -5.0,
    }
}

fn parse_date_inner(
    caller: &mut Caller<'_, WasmStore>,
    date_ptr: i32,
    date_len: i32,
    format_ptr: i32,
    format_len: i32,
    timezone_ptr: i32,
    timezone_len: i32,
) -> Result<f64> {
    let date = read_string(caller, date_ptr, date_len).context("invalid date string")?;
    let format = read_string(caller, format_ptr, format_len).context("invalid date format")?;
    let timezone = if timezone_len > 0 {
        read_string(caller, timezone_ptr, timezone_len)
    } else {
        None
    };
    let timezone = timezone
        .as_deref()
        .and_then(|value| value.parse().ok())
        .unwrap_or(chrono_tz::UTC);

    let format = swift_date_format_to_chrono(&format);
    let date = NaiveDateTime::parse_from_str(&date, &format)
        .ok()
        .and_then(|date| date.and_local_timezone(timezone).single())
        .context("failed to parse date")?;

    Ok(date.timestamp() as f64 + date.timestamp_subsec_nanos() as f64 / 1_000_000_000.0)
}

/// Converts a Swift `DateFormatter` format string to a `chrono` format string.
pub fn swift_date_format_to_chrono(format: &str) -> String {
    let mut result = String::new();
    let mut chars = format.chars().peekable();

    while let Some(character) = chars.next() {
        let token = match character {
            'y' => {
                let mut count = 1;
                while chars.peek() == Some(&'y') {
                    chars.next();
                    count += 1;
                }
                if count == 2 {
                    "%y"
                } else {
                    "%Y"
                }
            }
            'M' => {
                let mut count = 1;
                while chars.peek() == Some(&'M') {
                    chars.next();
                    count += 1;
                }
                match count {
                    1 | 2 => "%m",
                    3 => "%b",
                    _ => "%B",
                }
            }
            'd' => {
                while chars.peek() == Some(&'d') {
                    chars.next();
                }
                "%d"
            }
            'H' => {
                while chars.peek() == Some(&'H') {
                    chars.next();
                }
                "%H"
            }
            'h' => {
                while chars.peek() == Some(&'h') {
                    chars.next();
                }
                "%I"
            }
            'm' => {
                while chars.peek() == Some(&'m') {
                    chars.next();
                }
                "%M"
            }
            's' => {
                while chars.peek() == Some(&'s') {
                    chars.next();
                }
                "%S"
            }
            'S' => {
                while chars.peek() == Some(&'S') {
                    chars.next();
                }
                "%f"
            }
            'a' => "%p",
            'E' => {
                let mut count = 1;
                while chars.peek() == Some(&'E') {
                    chars.next();
                    count += 1;
                }
                if count >= 4 {
                    "%A"
                } else {
                    "%a"
                }
            }
            'z' => {
                let mut count = 1;
                while chars.peek() == Some(&'z') {
                    chars.next();
                    count += 1;
                }
                if count >= 4 {
                    "%Z"
                } else {
                    "%z"
                }
            }
            'Z' => {
                while chars.peek() == Some(&'Z') {
                    chars.next();
                }
                "%Z"
            }
            '\'' => {
                while let Some(quoted) = chars.next() {
                    if quoted == '\'' {
                        break;
                    }
                    result.push(quoted);
                }
                continue;
            }
            other => {
                result.push(other);
                continue;
            }
        };
        result.push_str(token);
    }

    result
}
