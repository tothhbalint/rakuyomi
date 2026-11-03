use scraper::Selector;
use wasmi::Caller;

use super::{
    html_dom::{HtmlDocument, HtmlElement, NodeKind},
    mem::read_string,
    store::{StoreItem, WasmStore},
};

const INVALID_DESCRIPTOR: i32 = -1;
const INVALID_STRING: i32 = -2;
const INVALID_QUERY: i32 = -4;
const NO_RESULT: i32 = -5;

fn store_string(caller: &mut Caller<'_, WasmStore>, value: String) -> i32 {
    caller.data_mut().store.store_string(value)
}

pub fn parse(
    mut caller: Caller<'_, WasmStore>,
    html_ptr: i32,
    html_len: i32,
    base_ptr: i32,
    base_len: i32,
) -> i32 {
    let Some(html) = read_string(&mut caller, html_ptr, html_len) else {
        return INVALID_STRING;
    };
    let base = if base_len > 0 {
        read_string(&mut caller, base_ptr, base_len)
    } else {
        None
    };
    let document = HtmlDocument::parse(&html, base.as_deref());
    caller
        .data_mut()
        .store
        .store(StoreItem::HtmlDocument(document))
}

pub fn parse_fragment(
    mut caller: Caller<'_, WasmStore>,
    html_ptr: i32,
    html_len: i32,
    base_ptr: i32,
    base_len: i32,
) -> i32 {
    let Some(html) = read_string(&mut caller, html_ptr, html_len) else {
        return INVALID_STRING;
    };
    let base = if base_len > 0 {
        read_string(&mut caller, base_ptr, base_len)
    } else {
        None
    };
    let document = HtmlDocument::parse_fragment(&html, base.as_deref());
    caller
        .data_mut()
        .store
        .store(StoreItem::HtmlDocument(document))
}

pub fn escape(mut caller: Caller<'_, WasmStore>, text_ptr: i32, text_len: i32) -> i32 {
    let Some(text) = read_string(&mut caller, text_ptr, text_len) else {
        return INVALID_STRING;
    };
    let escaped = html_escape::encode_safe(&text).to_string();
    store_string(&mut caller, escaped)
}

pub fn unescape(mut caller: Caller<'_, WasmStore>, text_ptr: i32, text_len: i32) -> i32 {
    let Some(text) = read_string(&mut caller, text_ptr, text_len) else {
        return INVALID_STRING;
    };
    let unescaped = html_escape::decode_html_entities(&text).to_string();
    store_string(&mut caller, unescaped)
}

pub fn kind(caller: Caller<'_, WasmStore>, rid: i32) -> i32 {
    let Some(item) = caller.data().store.get(rid) else {
        return INVALID_DESCRIPTOR;
    };

    let kind = match item {
        StoreItem::HtmlDocument(_) => NodeKind::Document,
        StoreItem::HtmlElement(element) => element.0.node_kind(),
        StoreItem::HtmlNode(node) => node.node_kind(),
        StoreItem::HtmlElementList(_) | StoreItem::HtmlNodeList(_) => NodeKind::ElementList,
        _ => NodeKind::Unknown,
    };
    kind.into()
}

pub fn child_nodes(mut caller: Caller<'_, WasmStore>, rid: i32) -> i32 {
    let Some(nodes) = caller
        .data()
        .store
        .get(rid)
        .and_then(|item| item.as_html_node())
        .and_then(|node| node.child_nodes())
    else {
        return INVALID_DESCRIPTOR;
    };
    caller
        .data_mut()
        .store
        .store(StoreItem::HtmlNodeList(nodes))
}

pub fn has_attr(mut caller: Caller<'_, WasmStore>, rid: i32, attr_ptr: i32, attr_len: i32) -> i32 {
    let Some(attr) = read_string(&mut caller, attr_ptr, attr_len) else {
        return 0;
    };
    let value = caller
        .data()
        .store
        .get(rid)
        .and_then(|item| item.as_html_element())
        .map(|element| element.has_attr(&attr))
        .unwrap_or(false);
    i32::from(value)
}

pub fn set_attr(
    _caller: Caller<'_, WasmStore>,
    _rid: i32,
    _key_ptr: i32,
    _key_len: i32,
    _value_ptr: i32,
    _value_len: i32,
) -> i32 {
    // The reference implementation doesn't support mutating the DOM either.
    INVALID_DESCRIPTOR
}

pub fn remove_attr(
    _caller: Caller<'_, WasmStore>,
    _rid: i32,
    _attr_ptr: i32,
    _attr_len: i32,
) -> i32 {
    INVALID_DESCRIPTOR
}

pub fn set_text(_caller: Caller<'_, WasmStore>, _rid: i32, _text_ptr: i32, _text_len: i32) -> i32 {
    INVALID_DESCRIPTOR
}

pub fn set_html(_caller: Caller<'_, WasmStore>, _rid: i32, _html_ptr: i32, _html_len: i32) -> i32 {
    INVALID_DESCRIPTOR
}

pub fn prepend(_caller: Caller<'_, WasmStore>, _rid: i32, _html_ptr: i32, _html_len: i32) -> i32 {
    INVALID_DESCRIPTOR
}

pub fn append(_caller: Caller<'_, WasmStore>, _rid: i32, _html_ptr: i32, _html_len: i32) -> i32 {
    INVALID_DESCRIPTOR
}

pub fn children(mut caller: Caller<'_, WasmStore>, rid: i32) -> i32 {
    let Some(elements) = caller
        .data()
        .store
        .get(rid)
        .and_then(|item| item.as_html_element())
        .and_then(|element| element.children())
    else {
        return INVALID_DESCRIPTOR;
    };
    caller
        .data_mut()
        .store
        .store(StoreItem::HtmlElementList(elements))
}

pub fn base_uri(mut caller: Caller<'_, WasmStore>, rid: i32) -> i32 {
    let Some(uri) = caller
        .data()
        .store
        .get(rid)
        .and_then(|item| item.as_html_element())
        .and_then(|element| element.base_uri())
    else {
        return NO_RESULT;
    };
    store_string(&mut caller, uri)
}

pub fn own_text(mut caller: Caller<'_, WasmStore>, rid: i32) -> i32 {
    let Some(item) = caller.data().store.get(rid) else {
        return INVALID_DESCRIPTOR;
    };
    let text = if let Some(element) = item.as_html_element() {
        element.own_text()
    } else if let Some(elements) = item.as_html_element_list() {
        elements.own_text()
    } else if let Some(node) = item.as_html_node() {
        node.text()
    } else {
        None
    };
    match text {
        Some(text) => store_string(&mut caller, text),
        None => NO_RESULT,
    }
}

pub fn data(mut caller: Caller<'_, WasmStore>, rid: i32) -> i32 {
    let Some(item) = caller.data().store.get(rid) else {
        return INVALID_DESCRIPTOR;
    };
    let data = if let Some(element) = item.as_html_element() {
        element.text(false)
    } else {
        item.as_html_node().and_then(|node| node.data())
    };
    match data {
        Some(data) => store_string(&mut caller, data),
        None => NO_RESULT,
    }
}

pub fn id(mut caller: Caller<'_, WasmStore>, rid: i32) -> i32 {
    let Some(id) = caller
        .data()
        .store
        .get(rid)
        .and_then(|item| item.as_html_element())
        .and_then(|element| element.id())
    else {
        return NO_RESULT;
    };
    store_string(&mut caller, id)
}

pub fn tag_name(mut caller: Caller<'_, WasmStore>, rid: i32) -> i32 {
    let Some(tag) = caller
        .data()
        .store
        .get(rid)
        .and_then(|item| item.as_html_element())
        .and_then(|element| element.tag_name())
    else {
        return NO_RESULT;
    };
    store_string(&mut caller, tag)
}

pub fn class_name(mut caller: Caller<'_, WasmStore>, rid: i32) -> i32 {
    let Some(class) = caller
        .data()
        .store
        .get(rid)
        .and_then(|item| item.as_html_element())
        .and_then(|element| element.class_name())
    else {
        return NO_RESULT;
    };
    store_string(&mut caller, class)
}

pub fn has_class(
    mut caller: Caller<'_, WasmStore>,
    rid: i32,
    class_ptr: i32,
    class_len: i32,
) -> i32 {
    let Some(class) = read_string(&mut caller, class_ptr, class_len) else {
        return 0;
    };
    let value = caller
        .data()
        .store
        .get(rid)
        .and_then(|item| item.as_html_element())
        .map(|element| element.has_class(&class))
        .unwrap_or(false);
    i32::from(value)
}

pub fn add_class(
    _caller: Caller<'_, WasmStore>,
    _rid: i32,
    _class_ptr: i32,
    _class_len: i32,
) -> i32 {
    INVALID_DESCRIPTOR
}

pub fn remove_class(
    _caller: Caller<'_, WasmStore>,
    _rid: i32,
    _class_ptr: i32,
    _class_len: i32,
) -> i32 {
    INVALID_DESCRIPTOR
}

pub fn parent(mut caller: Caller<'_, WasmStore>, rid: i32) -> i32 {
    let Some(item) = caller.data().store.get(rid) else {
        return INVALID_DESCRIPTOR;
    };
    if let Some(element) = item.as_html_element() {
        let Some(parent) = element.parent() else {
            return NO_RESULT;
        };
        caller
            .data_mut()
            .store
            .store(StoreItem::HtmlElement(parent))
    } else if let Some(node) = item.as_html_node() {
        let Some(parent) = node.parent() else {
            return NO_RESULT;
        };
        caller.data_mut().store.store(StoreItem::HtmlNode(parent))
    } else {
        INVALID_DESCRIPTOR
    }
}

pub fn siblings(mut caller: Caller<'_, WasmStore>, rid: i32) -> i32 {
    let Some(item) = caller.data().store.get(rid) else {
        return INVALID_DESCRIPTOR;
    };
    if let Some(element) = item.as_html_element() {
        let Some(elements) = element.siblings() else {
            return NO_RESULT;
        };
        caller
            .data_mut()
            .store
            .store(StoreItem::HtmlElementList(elements))
    } else if let Some(node) = item.as_html_node() {
        let Some(nodes) = node.siblings() else {
            return NO_RESULT;
        };
        caller
            .data_mut()
            .store
            .store(StoreItem::HtmlNodeList(nodes))
    } else {
        INVALID_DESCRIPTOR
    }
}

pub fn next(mut caller: Caller<'_, WasmStore>, rid: i32) -> i32 {
    let Some(item) = caller.data().store.get(rid) else {
        return INVALID_DESCRIPTOR;
    };
    if let Some(element) = item.as_html_element() {
        let Some(next) = element.next_sibling() else {
            return NO_RESULT;
        };
        caller.data_mut().store.store(StoreItem::HtmlElement(next))
    } else if let Some(node) = item.as_html_node() {
        let Some(next) = node.next_sibling() else {
            return NO_RESULT;
        };
        caller.data_mut().store.store(StoreItem::HtmlNode(next))
    } else {
        INVALID_DESCRIPTOR
    }
}

pub fn previous(mut caller: Caller<'_, WasmStore>, rid: i32) -> i32 {
    let Some(item) = caller.data().store.get(rid) else {
        return INVALID_DESCRIPTOR;
    };
    if let Some(element) = item.as_html_element() {
        let Some(previous) = element.prev_sibling() else {
            return NO_RESULT;
        };
        caller
            .data_mut()
            .store
            .store(StoreItem::HtmlElement(previous))
    } else if let Some(node) = item.as_html_node() {
        let Some(previous) = node.prev_sibling() else {
            return NO_RESULT;
        };
        caller.data_mut().store.store(StoreItem::HtmlNode(previous))
    } else {
        INVALID_DESCRIPTOR
    }
}

pub fn first(mut caller: Caller<'_, WasmStore>, rid: i32) -> i32 {
    let Some(item) = caller.data().store.get(rid) else {
        return INVALID_DESCRIPTOR;
    };
    let element = if let Some(elements) = item.as_html_element_list() {
        elements.0.first().cloned()
    } else if let Some(nodes) = item.as_html_node_list() {
        nodes.first().cloned().map(HtmlElement)
    } else {
        None
    };
    match element {
        Some(element) => caller
            .data_mut()
            .store
            .store(StoreItem::HtmlElement(element)),
        None => INVALID_DESCRIPTOR,
    }
}

pub fn last(mut caller: Caller<'_, WasmStore>, rid: i32) -> i32 {
    let Some(item) = caller.data().store.get(rid) else {
        return INVALID_DESCRIPTOR;
    };
    let element = if let Some(elements) = item.as_html_element_list() {
        elements.0.last().cloned()
    } else if let Some(nodes) = item.as_html_node_list() {
        nodes.last().cloned().map(HtmlElement)
    } else {
        None
    };
    match element {
        Some(element) => caller
            .data_mut()
            .store
            .store(StoreItem::HtmlElement(element)),
        None => INVALID_DESCRIPTOR,
    }
}

pub fn get(mut caller: Caller<'_, WasmStore>, rid: i32, index: i32) -> i32 {
    if index < 0 {
        return INVALID_DESCRIPTOR;
    }
    let Some(item) = caller.data().store.get(rid) else {
        return INVALID_DESCRIPTOR;
    };
    let element = if let Some(elements) = item.as_html_element_list() {
        elements.0.get(index as usize).cloned()
    } else if let Some(nodes) = item.as_html_node_list() {
        nodes.get(index as usize).cloned().map(HtmlElement)
    } else {
        None
    };
    match element {
        Some(element) => caller
            .data_mut()
            .store
            .store(StoreItem::HtmlElement(element)),
        None => INVALID_DESCRIPTOR,
    }
}

pub fn size(caller: Caller<'_, WasmStore>, rid: i32) -> i32 {
    let Some(item) = caller.data().store.get(rid) else {
        return INVALID_DESCRIPTOR;
    };
    if let Some(elements) = item.as_html_element_list() {
        elements.0.len() as i32
    } else if let Some(nodes) = item.as_html_node_list() {
        nodes.len() as i32
    } else {
        INVALID_DESCRIPTOR
    }
}

pub fn attr(mut caller: Caller<'_, WasmStore>, rid: i32, key_ptr: i32, key_len: i32) -> i32 {
    let Some(key) = read_string(&mut caller, key_ptr, key_len) else {
        return INVALID_STRING;
    };
    let Some(item) = caller.data().store.get(rid) else {
        return INVALID_DESCRIPTOR;
    };
    let value = if let Some(element) = item.as_html_element() {
        element.attr(&key)
    } else if let Some(elements) = item.as_html_element_list() {
        elements.attr(&key)
    } else {
        None
    };
    match value {
        Some(value) => store_string(&mut caller, value),
        None => NO_RESULT,
    }
}

pub fn outer_html(mut caller: Caller<'_, WasmStore>, rid: i32) -> i32 {
    let Some(item) = caller.data().store.get(rid) else {
        return INVALID_DESCRIPTOR;
    };
    let value = if let Some(element) = item.as_html_element() {
        element.outer_html()
    } else if let Some(elements) = item.as_html_element_list() {
        elements.outer_html()
    } else {
        None
    };
    match value {
        Some(value) => store_string(&mut caller, value),
        None => NO_RESULT,
    }
}

pub fn remove(_caller: Caller<'_, WasmStore>, _rid: i32) -> i32 {
    INVALID_DESCRIPTOR
}

pub fn select(mut caller: Caller<'_, WasmStore>, rid: i32, query_ptr: i32, query_len: i32) -> i32 {
    let Some(query) = read_string(&mut caller, query_ptr, query_len) else {
        return INVALID_STRING;
    };
    let Ok(selector) = Selector::parse(&query) else {
        return INVALID_QUERY;
    };
    let Some(item) = caller.data().store.get(rid) else {
        return INVALID_DESCRIPTOR;
    };

    let elements = if let Some(document) = item.as_html_document() {
        Some(document.select(&selector))
    } else if let Some(element) = item.as_html_element() {
        element.select(&selector)
    } else if let Some(elements) = item.as_html_element_list() {
        elements.select(&selector)
    } else {
        None
    };

    match elements {
        Some(elements) => caller
            .data_mut()
            .store
            .store(StoreItem::HtmlElementList(elements)),
        None => NO_RESULT,
    }
}

pub fn select_first(
    mut caller: Caller<'_, WasmStore>,
    rid: i32,
    query_ptr: i32,
    query_len: i32,
) -> i32 {
    let Some(query) = read_string(&mut caller, query_ptr, query_len) else {
        return INVALID_STRING;
    };
    let Ok(selector) = Selector::parse(&query) else {
        return INVALID_QUERY;
    };
    let Some(item) = caller.data().store.get(rid) else {
        return INVALID_DESCRIPTOR;
    };

    let element = if let Some(document) = item.as_html_document() {
        document.select(&selector).0.into_iter().next()
    } else if let Some(element) = item.as_html_element() {
        element.select_first(&selector)
    } else if let Some(elements) = item.as_html_element_list() {
        elements.select_first(&selector)
    } else {
        None
    };

    match element {
        Some(element) => caller
            .data_mut()
            .store
            .store(StoreItem::HtmlElement(element)),
        None => NO_RESULT,
    }
}

pub fn text(mut caller: Caller<'_, WasmStore>, rid: i32) -> i32 {
    let Some(item) = caller.data().store.get(rid) else {
        return INVALID_DESCRIPTOR;
    };
    let text = if let Some(element) = item.as_html_element() {
        element.text(true)
    } else if let Some(elements) = item.as_html_element_list() {
        elements.text(true)
    } else if let Some(node) = item.as_html_node() {
        node.text()
    } else {
        None
    };
    match text {
        Some(text) => store_string(&mut caller, text),
        None => NO_RESULT,
    }
}

pub fn untrimmed_text(mut caller: Caller<'_, WasmStore>, rid: i32) -> i32 {
    let Some(item) = caller.data().store.get(rid) else {
        return INVALID_DESCRIPTOR;
    };
    let text = if let Some(element) = item.as_html_element() {
        element.text(false)
    } else if let Some(elements) = item.as_html_element_list() {
        elements.text(false)
    } else {
        None
    };
    match text {
        Some(text) => store_string(&mut caller, text),
        None => NO_RESULT,
    }
}

pub fn html(mut caller: Caller<'_, WasmStore>, rid: i32) -> i32 {
    let Some(item) = caller.data().store.get(rid) else {
        return INVALID_DESCRIPTOR;
    };
    let value = if let Some(element) = item.as_html_element() {
        element.html()
    } else if let Some(elements) = item.as_html_element_list() {
        elements.html()
    } else {
        None
    };
    match value {
        Some(value) => store_string(&mut caller, value),
        None => NO_RESULT,
    }
}
