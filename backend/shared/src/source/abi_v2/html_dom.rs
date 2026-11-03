//! A small HTML DOM helper built on top of `scraper`.
//!
//! The current Aidoku ABI exposes HTML documents, elements, text nodes and element
//! lists as separate descriptors. This module owns the shared immutable document
//! and the node handles used by those descriptors.

use std::{ops::Deref, sync::Arc};

use ego_tree::NodeId;
use scraper::{CaseSensitivity, ElementRef, Html, Selector};
use url::Url;

#[derive(Debug, Clone)]
pub struct HtmlDocument {
    pub html: Arc<Html>,
    pub base_uri: Option<Arc<Url>>,
}

// `scraper::Html` uses non-atomic reference counting and interior mutability. All
// access to it is serialized behind the source mutex and the source runtime is
// only ever entered from one thread at a time.
unsafe impl Send for HtmlDocument {}
unsafe impl Sync for HtmlDocument {}

#[derive(Debug, Clone)]
pub struct HtmlNode {
    pub html: Arc<Html>,
    pub base_uri: Option<Arc<Url>>,
    pub id: NodeId,
}

unsafe impl Send for HtmlNode {}
unsafe impl Sync for HtmlNode {}

#[derive(Debug, Clone)]
pub struct HtmlElement(pub HtmlNode);

unsafe impl Send for HtmlElement {}
unsafe impl Sync for HtmlElement {}

#[derive(Debug, Clone)]
pub struct HtmlElementList(pub Vec<HtmlElement>);

unsafe impl Send for HtmlElementList {}
unsafe impl Sync for HtmlElementList {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Unknown,
    Node,
    TextNode,
    DataNode,
    Comment,
    Element,
    ElementList,
    Document,
}

impl From<NodeKind> for i32 {
    fn from(value: NodeKind) -> Self {
        match value {
            NodeKind::Unknown => 0,
            NodeKind::Node => 1,
            NodeKind::TextNode => 2,
            NodeKind::DataNode => 3,
            NodeKind::Comment => 4,
            NodeKind::Element => 5,
            NodeKind::ElementList => 6,
            NodeKind::Document => 7,
        }
    }
}

impl HtmlDocument {
    pub fn parse(html: &str, base_uri: Option<&str>) -> Self {
        Self::from_html(Html::parse_document(html), base_uri)
    }

    pub fn parse_fragment(html: &str, base_uri: Option<&str>) -> Self {
        Self::from_html(Html::parse_fragment(html), base_uri)
    }

    fn from_html(html: Html, base_uri: Option<&str>) -> Self {
        let base_uri = base_uri
            .filter(|value| !value.is_empty())
            .and_then(|value| Url::parse(value).ok())
            .map(Arc::new);
        Self {
            html: Arc::new(html),
            base_uri,
        }
    }

    pub fn root(&self) -> HtmlElement {
        HtmlElement(HtmlNode {
            html: self.html.clone(),
            base_uri: self.base_uri.clone(),
            id: self.html.tree.root().id(),
        })
    }

    pub fn select(&self, selector: &Selector) -> HtmlElementList {
        let elements = self
            .html
            .select_with_root(selector)
            .map(|element| {
                HtmlElement(HtmlNode {
                    html: self.html.clone(),
                    base_uri: self.base_uri.clone(),
                    id: element.id(),
                })
            })
            .collect();
        HtmlElementList(elements)
    }
}

impl HtmlNode {
    pub fn node_kind(&self) -> NodeKind {
        let Some(node) = self.html.tree.get(self.id) else {
            return NodeKind::Unknown;
        };
        match node.value() {
            scraper::node::Node::Document => NodeKind::Document,
            scraper::node::Node::Element(_) => NodeKind::Element,
            scraper::node::Node::Text(_) => NodeKind::TextNode,
            scraper::node::Node::Comment(_) => NodeKind::Comment,
            scraper::node::Node::Doctype(_) => NodeKind::DataNode,
            scraper::node::Node::ProcessingInstruction(_) => NodeKind::DataNode,
            _ => NodeKind::Node,
        }
    }

    fn child_node(&self, id: NodeId) -> HtmlNode {
        HtmlNode {
            html: self.html.clone(),
            base_uri: self.base_uri.clone(),
            id,
        }
    }

    pub fn as_element(&self) -> Option<HtmlElement> {
        if self.node_kind() == NodeKind::Element || self.node_kind() == NodeKind::Document {
            Some(HtmlElement(self.clone()))
        } else {
            None
        }
    }

    pub fn child_nodes(&self) -> Option<Vec<HtmlNode>> {
        let node = self.html.tree.get(self.id)?;
        Some(
            node.children()
                .map(|child| self.child_node(child.id()))
                .collect(),
        )
    }

    pub fn parent(&self) -> Option<HtmlNode> {
        let node = self.html.tree.get(self.id)?;
        node.parent().map(|parent| self.child_node(parent.id()))
    }

    pub fn siblings(&self) -> Option<Vec<HtmlNode>> {
        let node = self.html.tree.get(self.id)?;
        Some(
            node.next_siblings()
                .map(|sibling| self.child_node(sibling.id()))
                .collect(),
        )
    }

    pub fn next_sibling(&self) -> Option<HtmlNode> {
        let node = self.html.tree.get(self.id)?;
        node.next_sibling()
            .map(|sibling| self.child_node(sibling.id()))
    }

    pub fn prev_sibling(&self) -> Option<HtmlNode> {
        let node = self.html.tree.get(self.id)?;
        node.prev_sibling()
            .map(|sibling| self.child_node(sibling.id()))
    }

    pub fn text(&self) -> Option<String> {
        let node = self.html.tree.get(self.id)?;
        node.value().as_text().map(|text| text.deref().to_string())
    }

    pub fn data(&self) -> Option<String> {
        let node = self.html.tree.get(self.id)?;
        match node.value() {
            scraper::node::Node::Text(text) => Some(text.deref().to_string()),
            scraper::node::Node::Comment(comment) => Some(comment.deref().to_string()),
            scraper::node::Node::Doctype(_) | scraper::node::Node::ProcessingInstruction(_) => None,
            scraper::node::Node::Element(element) => Some(element.name().to_string()),
            _ => None,
        }
    }
}

fn normalize_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

impl HtmlElement {
    pub fn select(&self, selector: &Selector) -> Option<HtmlElementList> {
        let node = self.0.html.tree.get(self.0.id)?;
        let element = ElementRef::wrap(node)?;
        let elements = element
            .select_with_root(selector)
            .map(|element| self.child(element.id()))
            .collect();
        Some(HtmlElementList(elements))
    }

    pub fn select_first(&self, selector: &Selector) -> Option<HtmlElement> {
        let node = self.0.html.tree.get(self.0.id)?;
        let element = ElementRef::wrap(node)?;
        element
            .select_with_root(selector)
            .next()
            .map(|element| self.child(element.id()))
    }

    pub fn attr(&self, name: &str) -> Option<String> {
        let has_abs_prefix = name.starts_with("abs:");
        let name = if has_abs_prefix { &name[4..] } else { name };
        let node = self.0.html.tree.get(self.0.id)?;
        let element = ElementRef::wrap(node)?;
        let attr = element.attr(name).map(|value| value.to_string());
        if has_abs_prefix {
            attr.and_then(|value| {
                Url::parse(&value)
                    .ok()
                    .map(|url| url.to_string())
                    .or_else(|| {
                        self.0
                            .base_uri
                            .as_ref()
                            .and_then(|base| base.join(&value).ok())
                            .map(|url| url.to_string())
                    })
            })
        } else {
            attr
        }
    }

    pub fn text(&self, trimmed: bool) -> Option<String> {
        let element = ElementRef::wrap(self.0.html.tree.get(self.0.id)?)?;
        let text = element.text().collect::<Vec<_>>().join(" ");
        if trimmed {
            Some(normalize_text(&text))
        } else {
            Some(text)
        }
    }

    pub fn own_text(&self) -> Option<String> {
        let node = self.0.html.tree.get(self.0.id)?;
        let element = ElementRef::wrap(node)?;
        let text = element
            .children()
            .filter_map(|child| match child.value() {
                scraper::node::Node::Text(text) => Some(text.deref().to_string()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" ");
        Some(normalize_text(&text))
    }

    pub fn html(&self) -> Option<String> {
        let node = self.0.html.tree.get(self.0.id)?;
        let element = ElementRef::wrap(node)?;
        Some(element.inner_html())
    }

    pub fn outer_html(&self) -> Option<String> {
        let node = self.0.html.tree.get(self.0.id)?;
        let element = ElementRef::wrap(node)?;
        Some(element.html())
    }

    pub fn parent(&self) -> Option<HtmlElement> {
        let node = self.0.html.tree.get(self.0.id)?;
        let element = ElementRef::wrap(node)?;
        element.parent().map(|parent| self.child(parent.id()))
    }

    pub fn children(&self) -> Option<HtmlElementList> {
        let node = self.0.html.tree.get(self.0.id)?;
        let element = ElementRef::wrap(node)?;
        Some(HtmlElementList(
            element
                .child_elements()
                .map(|child| self.child(child.id()))
                .collect(),
        ))
    }

    pub fn siblings(&self) -> Option<HtmlElementList> {
        let node = self.0.html.tree.get(self.0.id)?;
        let element = ElementRef::wrap(node)?;
        Some(HtmlElementList(
            element
                .next_siblings()
                .filter_map(|sibling| ElementRef::wrap(sibling))
                .map(|sibling| self.child(sibling.id()))
                .collect(),
        ))
    }

    pub fn next_sibling(&self) -> Option<HtmlElement> {
        let node = self.0.html.tree.get(self.0.id)?;
        let element = ElementRef::wrap(node)?;
        element
            .next_siblings()
            .filter_map(|sibling| ElementRef::wrap(sibling))
            .next()
            .map(|sibling| self.child(sibling.id()))
    }

    pub fn prev_sibling(&self) -> Option<HtmlElement> {
        let node = self.0.html.tree.get(self.0.id)?;
        let element = ElementRef::wrap(node)?;
        element
            .prev_siblings()
            .filter_map(|sibling| ElementRef::wrap(sibling))
            .next()
            .map(|sibling| self.child(sibling.id()))
    }

    pub fn id(&self) -> Option<String> {
        let node = self.0.html.tree.get(self.0.id)?;
        let element = ElementRef::wrap(node)?;
        element.value().id().map(|value| value.to_string())
    }

    pub fn tag_name(&self) -> Option<String> {
        let node = self.0.html.tree.get(self.0.id)?;
        let element = ElementRef::wrap(node)?;
        Some(element.value().name().to_string())
    }

    pub fn class_name(&self) -> Option<String> {
        let node = self.0.html.tree.get(self.0.id)?;
        let element = ElementRef::wrap(node)?;
        Some(element.value().classes().collect::<Vec<&str>>().join(" "))
    }

    pub fn has_class(&self, class: &str) -> bool {
        let Some(node) = self.0.html.tree.get(self.0.id) else {
            return false;
        };
        let Some(element) = ElementRef::wrap(node) else {
            return false;
        };
        element
            .value()
            .has_class(class, CaseSensitivity::AsciiCaseInsensitive)
    }

    pub fn has_attr(&self, name: &str) -> bool {
        let Some(node) = self.0.html.tree.get(self.0.id) else {
            return false;
        };
        let Some(element) = ElementRef::wrap(node) else {
            return false;
        };
        element.value().attrs().any(|(key, _)| key == name)
    }

    pub fn base_uri(&self) -> Option<String> {
        self.0.base_uri.as_ref().map(|value| value.to_string())
    }

    fn child(&self, id: NodeId) -> HtmlElement {
        HtmlElement(HtmlNode {
            html: self.0.html.clone(),
            base_uri: self.0.base_uri.clone(),
            id,
        })
    }
}

impl HtmlElementList {
    pub fn select(&self, selector: &Selector) -> Option<HtmlElementList> {
        Some(HtmlElementList(
            self.0
                .iter()
                .filter_map(|element| element.select(selector))
                .flat_map(|elements| elements.0)
                .collect(),
        ))
    }

    pub fn select_first(&self, selector: &Selector) -> Option<HtmlElement> {
        self.0
            .iter()
            .find_map(|element| element.select_first(selector))
    }

    pub fn attr(&self, name: &str) -> Option<String> {
        self.0.iter().find_map(|element| element.attr(name))
    }

    pub fn text(&self, trimmed: bool) -> Option<String> {
        let text = self
            .0
            .iter()
            .filter_map(|element| element.text(trimmed))
            .collect::<Vec<_>>()
            .join(" ");
        if trimmed {
            Some(normalize_text(&text))
        } else {
            Some(text)
        }
    }

    pub fn own_text(&self) -> Option<String> {
        Some(
            self.0
                .iter()
                .filter_map(|element| element.own_text())
                .collect::<Vec<_>>()
                .join(" "),
        )
    }

    pub fn html(&self) -> Option<String> {
        Some(
            self.0
                .iter()
                .filter_map(|element| element.html())
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }

    pub fn outer_html(&self) -> Option<String> {
        Some(
            self.0
                .iter()
                .filter_map(|element| element.outer_html())
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }
}

pub trait SelectWithRoot<'a, 'b> {
    fn select_with_root(&'a self, selector: &'b Selector) -> SelectRoot<'a, 'b>;
}

pub struct SelectRoot<'a, 'b> {
    root: Option<std::iter::Once<ElementRef<'a>>>,
    select_el: Option<scraper::element_ref::Select<'a, 'b>>,
    select_doc: Option<scraper::html::Select<'a, 'b>>,
}

impl<'a> Iterator for SelectRoot<'a, '_> {
    type Item = ElementRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(root) = self.root.as_mut() {
            if let Some(element) = root.next() {
                return Some(element);
            }
            self.root = None;
        }
        if let Some(select) = self.select_el.as_mut() {
            return select.next();
        }
        if let Some(select) = self.select_doc.as_mut() {
            return select.next();
        }
        None
    }
}

impl<'a, 'b> SelectWithRoot<'a, 'b> for Html {
    fn select_with_root(&'a self, selector: &'b Selector) -> SelectRoot<'a, 'b> {
        let root_node = self.tree.root();
        let root = ElementRef::wrap(root_node).filter(|element| selector.matches(element));
        SelectRoot {
            root: root.map(std::iter::once),
            select_el: None,
            select_doc: Some(self.select(selector)),
        }
    }
}

impl<'a, 'b> SelectWithRoot<'a, 'b> for ElementRef<'a> {
    fn select_with_root(&'a self, selector: &'b Selector) -> SelectRoot<'a, 'b> {
        let root = selector.matches(self).then(|| std::iter::once(*self));
        SelectRoot {
            root,
            select_el: Some(self.select(selector)),
            select_doc: None,
        }
    }
}
