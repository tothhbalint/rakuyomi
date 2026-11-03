use std::time::Duration;

use anyhow::{Context, Result};
use log::warn;
use reqwest::{
    blocking::Client,
    header::{HeaderName, HeaderValue},
};
use url::Url;
use wasmi::Caller;

use super::{
    html_dom::HtmlDocument,
    mem::{read_bytes, read_string, write_bytes},
    store::{HttpMethod, NetRequest, NetResponse, StoreItem, WasmStore},
};

pub const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:109.0) Gecko/20100101 Firefox/115.0";

const INVALID_DESCRIPTOR: i32 = -1;
const INVALID_STRING: i32 = -2;
const INVALID_METHOD: i32 = -3;
const INVALID_URL: i32 = -4;
const MISSING_DATA: i32 = -7;
const MISSING_RESPONSE: i32 = -8;
const REQUEST_ERROR: i32 = -10;
const FAILED_MEMORY_WRITE: i32 = -11;
const NOT_AN_IMAGE: i32 = -12;

pub fn init(mut caller: Caller<'_, WasmStore>, method: i32) -> i32 {
    let Some(method) = HttpMethod::from_abi(method) else {
        return INVALID_METHOD;
    };
    let request = NetRequest::new(method);
    caller
        .data_mut()
        .store
        .store(StoreItem::Request(Box::new(request)))
}

fn execute(caller: &mut Caller<'_, WasmStore>, rid: i32) -> i32 {
    let Some(request) = caller
        .data_mut()
        .store
        .get_mut(rid)
        .and_then(|item| item.as_request_mut())
    else {
        return INVALID_DESCRIPTOR;
    };

    let Some(url) = request.url.clone() else {
        return INVALID_URL;
    };

    if !request.headers.contains_key(reqwest::header::USER_AGENT) {
        if let Ok(value) = HeaderValue::from_str(DEFAULT_USER_AGENT) {
            request.headers.insert(reqwest::header::USER_AGENT, value);
        }
    }

    let mut builder = Client::new()
        .request(request.method.into_reqwest(), url.clone())
        .headers(request.headers.clone());

    if let Some(timeout) = request.timeout {
        builder = builder.timeout(timeout);
    }
    if let Some(body) = request.body.take() {
        builder = builder.body(body);
    }

    let response = match builder.send() {
        Ok(response) => response,
        Err(error) => {
            warn!("request to {} failed: {}", url, error);
            return REQUEST_ERROR;
        }
    };

    let response_url = response.url().clone();
    let status = response.status();
    let headers = response.headers().clone();
    let data = match response.bytes() {
        Ok(data) => data.to_vec(),
        Err(error) => {
            warn!("failed to read response from {}: {}", url, error);
            return REQUEST_ERROR;
        }
    };

    request.response = Some(NetResponse {
        url: response_url,
        status,
        headers,
        data,
    });

    0
}

pub fn send(mut caller: Caller<'_, WasmStore>, rid: i32) -> i32 {
    execute(&mut caller, rid)
}

pub fn send_all(mut caller: Caller<'_, WasmStore>, pointer: i32, length: i32) -> i32 {
    if length <= 0 {
        return 0;
    }
    let Some(bytes) = read_bytes(&mut caller, pointer, length * 4) else {
        return INVALID_DESCRIPTOR;
    };
    let mut rids = bytes
        .chunks_exact(4)
        .map(|chunk| i32::from_le_bytes(chunk.try_into().unwrap()))
        .collect::<Vec<_>>();

    let mut had_error = false;
    for rid in rids.iter_mut() {
        let result = execute(&mut caller, *rid);
        if result != 0 {
            had_error = true;
        }
        *rid = result;
    }

    let result_bytes = rids
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect::<Vec<_>>();
    if write_bytes(&mut caller, pointer, &result_bytes).is_none() {
        return FAILED_MEMORY_WRITE;
    }

    if had_error {
        REQUEST_ERROR
    } else {
        0
    }
}

pub fn set_url(mut caller: Caller<'_, WasmStore>, rid: i32, pointer: i32, length: i32) -> i32 {
    let Some(url) = read_string(&mut caller, pointer, length) else {
        return INVALID_STRING;
    };
    let Ok(url) = Url::parse(&url) else {
        return INVALID_URL;
    };
    let Some(request) = caller
        .data_mut()
        .store
        .get_mut(rid)
        .and_then(|item| item.as_request_mut())
    else {
        return INVALID_DESCRIPTOR;
    };
    request.url = Some(url);
    0
}

pub fn set_header(
    mut caller: Caller<'_, WasmStore>,
    rid: i32,
    key_ptr: i32,
    key_len: i32,
    value_ptr: i32,
    value_len: i32,
) -> i32 {
    let Some(key) = read_string(&mut caller, key_ptr, key_len) else {
        return INVALID_STRING;
    };
    let Some(value) = read_string(&mut caller, value_ptr, value_len) else {
        return INVALID_STRING;
    };
    let Ok(name) = HeaderName::from_bytes(key.as_bytes()) else {
        return INVALID_STRING;
    };
    let Ok(value) = HeaderValue::from_str(&value) else {
        return INVALID_STRING;
    };
    let Some(request) = caller
        .data_mut()
        .store
        .get_mut(rid)
        .and_then(|item| item.as_request_mut())
    else {
        return INVALID_DESCRIPTOR;
    };
    request.headers.insert(name, value);
    0
}

pub fn set_body(mut caller: Caller<'_, WasmStore>, rid: i32, pointer: i32, length: i32) -> i32 {
    let Some(body) = read_bytes(&mut caller, pointer, length) else {
        return INVALID_STRING;
    };
    let Some(request) = caller
        .data_mut()
        .store
        .get_mut(rid)
        .and_then(|item| item.as_request_mut())
    else {
        return INVALID_DESCRIPTOR;
    };
    request.body = Some(body);
    0
}

pub fn set_timeout(mut caller: Caller<'_, WasmStore>, rid: i32, value: f64) -> i32 {
    if !value.is_finite() || value < 0.0 {
        return INVALID_STRING;
    }
    let Some(request) = caller
        .data_mut()
        .store
        .get_mut(rid)
        .and_then(|item| item.as_request_mut())
    else {
        return INVALID_DESCRIPTOR;
    };
    request.timeout = Some(Duration::from_secs_f64(value));
    0
}

pub fn data_len(caller: Caller<'_, WasmStore>, rid: i32) -> i32 {
    let Some(response) = caller
        .data()
        .store
        .get(rid)
        .and_then(|item| item.as_request())
        .and_then(|request| request.response.as_ref())
    else {
        return MISSING_RESPONSE;
    };
    response.data.len() as i32
}

pub fn read_data(mut caller: Caller<'_, WasmStore>, rid: i32, pointer: i32, size: i32) -> i32 {
    let Some(data) = caller
        .data()
        .store
        .get(rid)
        .and_then(|item| item.as_request())
        .and_then(|request| request.response.as_ref())
        .map(|response| response.data.clone())
    else {
        return MISSING_RESPONSE;
    };

    if size < 0 || size as usize > data.len() {
        return FAILED_MEMORY_WRITE;
    }

    match write_bytes(&mut caller, pointer, &data[..size as usize]) {
        Some(()) => 0,
        None => FAILED_MEMORY_WRITE,
    }
}

pub fn get_image(_caller: Caller<'_, WasmStore>, _rid: i32) -> i32 {
    // Decoding images is only needed by sources that process page images. We
    // don't support that yet; returning NotAnImage is the same behaviour the
    // reference test runner has for unsupported operations.
    NOT_AN_IMAGE
}

pub fn get_status_code(caller: Caller<'_, WasmStore>, rid: i32) -> i32 {
    caller
        .data()
        .store
        .get(rid)
        .and_then(|item| item.as_request())
        .and_then(|request| request.response.as_ref())
        .map(|response| response.status.as_u16() as i32)
        .unwrap_or(MISSING_RESPONSE)
}

pub fn get_url(mut caller: Caller<'_, WasmStore>, rid: i32) -> i32 {
    let Some(url) = caller
        .data()
        .store
        .get(rid)
        .and_then(|item| item.as_request())
        .and_then(|request| request.response.as_ref())
        .map(|response| response.url.to_string())
    else {
        return MISSING_RESPONSE;
    };
    caller.data_mut().store.store_string(url)
}

pub fn get_header(mut caller: Caller<'_, WasmStore>, rid: i32, key_ptr: i32, key_len: i32) -> i32 {
    let Some(key) = read_string(&mut caller, key_ptr, key_len) else {
        return INVALID_STRING;
    };
    let Ok(name) = HeaderName::from_bytes(key.as_bytes()) else {
        return INVALID_STRING;
    };
    let Some(response) = caller
        .data()
        .store
        .get(rid)
        .and_then(|item| item.as_request())
        .and_then(|request| request.response.as_ref())
    else {
        return MISSING_RESPONSE;
    };
    let value = response
        .headers
        .get_all(&name)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .collect::<Vec<_>>()
        .join(", ");
    if value.is_empty() {
        return MISSING_DATA;
    }
    caller.data_mut().store.store_string(value)
}

pub fn html(mut caller: Caller<'_, WasmStore>, rid: i32) -> i32 {
    let Some((body, url)) = caller
        .data()
        .store
        .get(rid)
        .and_then(|item| item.as_request())
        .and_then(|request| request.response.as_ref())
        .map(|response| (response.data.clone(), response.url.clone()))
    else {
        return MISSING_RESPONSE;
    };

    let Ok(text) = String::from_utf8(body) else {
        return INVALID_STRING;
    };
    let document = HtmlDocument::parse(&text, Some(url.as_str()));
    caller
        .data_mut()
        .store
        .store(StoreItem::HtmlDocument(document))
}

pub fn set_rate_limit(_caller: Caller<'_, WasmStore>, _permits: i32, _period: i32, _unit: i32) {
    // Rate limiting is handled by the source itself in most cases. The reference
    // implementation also treats this as a no-op.
}

#[allow(dead_code)]
fn request_from_store(request: &NetRequest) -> Result<reqwest::blocking::Request> {
    let url = request.url.clone().context("request has no URL")?;
    let mut builder = reqwest::blocking::Request::new(request.method.into_reqwest(), url);
    *builder.headers_mut() = request.headers.clone();
    if let Some(body) = request.body.clone() {
        *builder.body_mut() = Some(body.into());
    }
    Ok(builder)
}
