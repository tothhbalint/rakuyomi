use std::{collections::HashMap, time::Duration};

use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    Method, StatusCode, Url,
};
use serde::{de::DeserializeOwned, Serialize};

use super::html_dom::{HtmlDocument, HtmlElement, HtmlElementList, HtmlNode};

#[derive(Debug, Clone)]
pub enum DefaultValue {
    Data(Vec<u8>),
    Bool(bool),
    Int(i32),
    Float(f32),
    String(String),
    StringArray(Vec<String>),
    Null,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Head,
    Delete,
    Patch,
    Options,
    Connect,
    Trace,
}

impl HttpMethod {
    pub fn from_abi(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::Get),
            1 => Some(Self::Post),
            2 => Some(Self::Put),
            3 => Some(Self::Head),
            4 => Some(Self::Delete),
            5 => Some(Self::Patch),
            6 => Some(Self::Options),
            7 => Some(Self::Connect),
            8 => Some(Self::Trace),
            _ => None,
        }
    }

    pub fn into_reqwest(self) -> Method {
        match self {
            Self::Get => Method::GET,
            Self::Post => Method::POST,
            Self::Put => Method::PUT,
            Self::Head => Method::HEAD,
            Self::Delete => Method::DELETE,
            Self::Patch => Method::PATCH,
            Self::Options => Method::OPTIONS,
            Self::Connect => Method::CONNECT,
            Self::Trace => Method::TRACE,
        }
    }
}

#[derive(Debug)]
pub struct NetResponse {
    pub url: Url,
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub struct NetRequest {
    pub method: HttpMethod,
    pub url: Option<Url>,
    pub headers: HeaderMap,
    pub body: Option<Vec<u8>>,
    pub timeout: Option<Duration>,
    pub response: Option<NetResponse>,
}

impl NetRequest {
    pub fn new(method: HttpMethod) -> Self {
        Self {
            method,
            url: None,
            headers: HeaderMap::new(),
            body: None,
            timeout: None,
            response: None,
        }
    }
}

#[derive(Debug)]
pub struct ImageData {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

pub enum StoreItem {
    String(String),
    Encoded(Vec<u8>),
    HtmlDocument(HtmlDocument),
    HtmlNode(HtmlNode),
    HtmlElement(HtmlElement),
    HtmlElementList(HtmlElementList),
    HtmlNodeList(Vec<HtmlNode>),
    Request(Box<NetRequest>),
    Image(ImageData),
}

impl StoreItem {
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::String(value) => Some(value.as_bytes()),
            Self::Encoded(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_html_document(&self) -> Option<&HtmlDocument> {
        match self {
            Self::HtmlDocument(document) => Some(document),
            _ => None,
        }
    }

    pub fn as_html_node(&self) -> Option<&HtmlNode> {
        match self {
            Self::HtmlNode(node) => Some(node),
            Self::HtmlElement(element) => Some(&element.0),
            _ => None,
        }
    }

    pub fn as_html_element(&self) -> Option<&HtmlElement> {
        match self {
            Self::HtmlElement(element) => Some(element),
            _ => None,
        }
    }

    pub fn as_html_element_list(&self) -> Option<&HtmlElementList> {
        match self {
            Self::HtmlElementList(elements) => Some(elements),
            _ => None,
        }
    }

    pub fn as_html_node_list(&self) -> Option<&[HtmlNode]> {
        match self {
            Self::HtmlNodeList(nodes) => Some(nodes),
            _ => None,
        }
    }

    pub fn as_request(&self) -> Option<&NetRequest> {
        match self {
            Self::Request(request) => Some(request),
            _ => None,
        }
    }

    pub fn as_request_mut(&mut self) -> Option<&mut NetRequest> {
        match self {
            Self::Request(request) => Some(request),
            _ => None,
        }
    }

    pub fn as_image(&self) -> Option<&ImageData> {
        match self {
            Self::Image(image) => Some(image),
            _ => None,
        }
    }
}

#[derive(Default)]
pub struct DescriptorStore {
    pointer: i32,
    items: HashMap<i32, StoreItem>,
}

impl DescriptorStore {
    pub fn new() -> Self {
        Self {
            pointer: 1,
            items: HashMap::new(),
        }
    }

    pub fn store(&mut self, item: StoreItem) -> i32 {
        let descriptor = self.pointer;
        self.pointer = self.pointer.wrapping_add(1).max(1);
        self.items.insert(descriptor, item);
        descriptor
    }

    pub fn store_encoded<T: Serialize>(&mut self, value: &T) -> i32 {
        match postcard::to_allocvec(value) {
            Ok(bytes) => self.store(StoreItem::Encoded(bytes)),
            Err(_) => -1,
        }
    }

    pub fn store_string(&mut self, value: String) -> i32 {
        self.store(StoreItem::String(value))
    }

    pub fn get(&self, descriptor: i32) -> Option<&StoreItem> {
        self.items.get(&descriptor)
    }

    pub fn get_mut(&mut self, descriptor: i32) -> Option<&mut StoreItem> {
        self.items.get_mut(&descriptor)
    }

    pub fn take(&mut self, descriptor: i32) -> Option<StoreItem> {
        self.items.remove(&descriptor)
    }

    pub fn remove(&mut self, descriptor: i32) {
        self.items.remove(&descriptor);
    }

    pub fn read<T: DeserializeOwned>(&self, descriptor: i32) -> Option<T> {
        let item = self.get(descriptor)?;
        match item {
            StoreItem::Encoded(bytes) => postcard::from_bytes(bytes).ok(),
            StoreItem::String(value) => postcard::from_bytes(value.as_bytes()).ok(),
            _ => None,
        }
    }

    pub fn read_string(&self, descriptor: i32) -> Option<String> {
        match self.get(descriptor)? {
            StoreItem::String(value) => {
                if value.is_empty() {
                    None
                } else {
                    Some(value.clone())
                }
            }
            StoreItem::Encoded(bytes) => {
                let value = String::from_utf8(bytes.clone()).ok()?;
                if value.is_empty() {
                    None
                } else {
                    Some(value)
                }
            }
            _ => None,
        }
    }
}

pub struct WasmStore {
    pub id: String,
    pub store: DescriptorStore,
    pub defaults: HashMap<String, DefaultValue>,
    pub cancellation_token: tokio_util::sync::CancellationToken,
}

impl WasmStore {
    pub fn new(
        id: String,
        defaults: HashMap<String, DefaultValue>,
        cancellation_token: tokio_util::sync::CancellationToken,
    ) -> Self {
        Self {
            id,
            store: DescriptorStore::new(),
            defaults,
            cancellation_token,
        }
    }
}

pub fn parse_header_name(value: &str) -> Option<HeaderName> {
    HeaderName::from_bytes(value.as_bytes()).ok()
}

pub fn parse_header_value(value: &str) -> Option<HeaderValue> {
    HeaderValue::from_str(value).ok()
}
