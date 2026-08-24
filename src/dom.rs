//! Nexus-owned DOM representation.
//!
//! `html5ever` parses HTML, but the resulting tree belongs to Nexus. In 0.6
//! the tree becomes mutable so the JavaScript bridge can change text and
//! attributes before style/layout are recomputed.

use url::Url;

use crate::address::resolve_url;

pub type NodeId = usize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomAttribute {
    pub namespace: Option<String>,
    pub prefix: Option<String>,
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomNodeData {
    Document,
    Doctype {
        name: String,
        public_id: String,
        system_id: String,
    },
    Element {
        namespace: String,
        prefix: Option<String>,
        tag_name: String,
        attributes: Vec<DomAttribute>,
    },
    Text(String),
    Comment(String),
    ProcessingInstruction {
        target: String,
        contents: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomNode {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub data: DomNodeData,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub node_id: NodeId,
    pub label: String,
    pub href: String,
    pub resolved_url: Option<Url>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageReference {
    pub node_id: NodeId,
    pub src: String,
    pub resolved_url: Option<Url>,
    pub alt: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptReference {
    pub node_id: NodeId,
    pub src: Option<String>,
    pub resolved_url: Option<Url>,
    pub inline_code: String,
    pub script_type: String,
    pub is_module: bool,
}

#[derive(Debug, Clone)]
pub struct NexusDom {
    document_url: Url,
    root: NodeId,
    nodes: Vec<DomNode>,
    parse_errors: Vec<String>,
}

impl NexusDom {
    pub(crate) fn new(
        document_url: Url,
        root: NodeId,
        nodes: Vec<DomNode>,
        parse_errors: Vec<String>,
    ) -> Self {
        Self {
            document_url,
            root,
            nodes,
            parse_errors,
        }
    }

    pub fn document_url(&self) -> &Url {
        &self.document_url
    }

    pub fn root(&self) -> NodeId {
        self.root
    }

    pub fn nodes(&self) -> &[DomNode] {
        &self.nodes
    }

    pub fn node(&self, id: NodeId) -> Option<&DomNode> {
        self.nodes.get(id)
    }

    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut DomNode> {
        self.nodes.get_mut(id)
    }

    pub fn parse_errors(&self) -> &[String] {
        &self.parse_errors
    }

    pub fn title(&self) -> Option<String> {
        let title = self.find_first_element("title")?;
        let text = self.text_content(title);
        let normalized = normalize_whitespace(&text);
        (!normalized.is_empty()).then_some(normalized)
    }

    pub fn body_text(&self) -> String {
        let start = self.find_first_element("body").unwrap_or(self.root);
        normalize_whitespace(&self.text_content(start))
    }

    pub fn base_url(&self) -> Url {
        self.base_href()
            .and_then(|href| resolve_url(&self.document_url, &href))
            .unwrap_or_else(|| self.document_url.clone())
    }

    pub fn images(&self) -> Vec<ImageReference> {
        let base = self.base_url();
        self.reachable_ids()
            .into_iter()
            .filter_map(|id| {
                let node = self.node(id)?;
                let DomNodeData::Element {
                    tag_name,
                    attributes,
                    ..
                } = &node.data
                else {
                    return None;
                };
                if !tag_name.eq_ignore_ascii_case("img") {
                    return None;
                }
                let src = attribute_value(attributes, "src")?.trim().to_owned();
                if src.is_empty() {
                    return None;
                }
                Some(ImageReference {
                    node_id: node.id,
                    resolved_url: resolve_url(&base, &src),
                    alt: attribute_value(attributes, "alt").unwrap_or("").to_owned(),
                    src,
                })
            })
            .collect()
    }

    pub fn links(&self) -> Vec<Link> {
        let base = self.base_url();
        self.reachable_ids()
            .into_iter()
            .filter_map(|id| {
                let node = self.node(id)?;
                let DomNodeData::Element {
                    tag_name,
                    attributes,
                    ..
                } = &node.data
                else {
                    return None;
                };
                if !tag_name.eq_ignore_ascii_case("a") {
                    return None;
                }
                let href = attribute_value(attributes, "href")?.to_owned();
                Some(Link {
                    node_id: node.id,
                    label: normalize_whitespace(&self.text_content(node.id)),
                    resolved_url: resolve_url(&base, &href),
                    href,
                })
            })
            .collect()
    }

    pub fn scripts(&self) -> Vec<ScriptReference> {
        let base = self.base_url();
        self.reachable_ids()
            .into_iter()
            .filter_map(|id| {
                let node = self.node(id)?;
                let DomNodeData::Element {
                    tag_name,
                    attributes,
                    ..
                } = &node.data
                else {
                    return None;
                };
                if !tag_name.eq_ignore_ascii_case("script") {
                    return None;
                }

                let script_type = attribute_value(attributes, "type")
                    .unwrap_or("")
                    .trim()
                    .to_ascii_lowercase();
                let is_module = script_type == "module";
                let src = attribute_value(attributes, "src")
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned);
                let resolved_url = src.as_deref().and_then(|value| resolve_url(&base, value));
                let inline_code = if src.is_none() {
                    self.text_content_raw(node.id)
                } else {
                    String::new()
                };

                Some(ScriptReference {
                    node_id: node.id,
                    src,
                    resolved_url,
                    inline_code,
                    script_type,
                    is_module,
                })
            })
            .collect()
    }

    pub fn pretty_tree(&self, max_depth: usize) -> String {
        let mut out = String::new();
        self.write_tree(self.root, 0, max_depth, &mut out);
        out
    }

    pub fn text_content(&self, id: NodeId) -> String {
        normalize_whitespace(&self.text_content_raw(id))
    }

    pub fn text_content_raw(&self, id: NodeId) -> String {
        let Some(node) = self.node(id) else {
            return String::new();
        };
        match &node.data {
            DomNodeData::Text(text) => text.clone(),
            DomNodeData::Comment(_) | DomNodeData::ProcessingInstruction { .. } => String::new(),
            _ => {
                let mut result = String::new();
                for &child in &node.children {
                    let piece = self.text_content_raw(child);
                    if !piece.is_empty() {
                        result.push_str(&piece);
                    }
                }
                result
            }
        }
    }

    pub fn find_first_element(&self, tag: &str) -> Option<NodeId> {
        self.reachable_ids().into_iter().find(|&id| {
            matches!(
                self.node(id).map(|node| &node.data),
                Some(DomNodeData::Element { tag_name, .. }) if tag_name.eq_ignore_ascii_case(tag)
            )
        })
    }

    pub fn find_element_by_id(&self, wanted: &str) -> Option<NodeId> {
        self.reachable_ids().into_iter().find(|&id| {
            self.element_tag_name(id).is_some() && self.attribute(id, "id") == Some(wanted)
        })
    }

    /// Minimal querySelector subset for Nexus 0.20: tag, #id, .class and simple
    /// compounds such as `div.card#main`.
    pub fn query_selector(&self, selector: &str) -> Option<NodeId> {
        let selector = SimpleDomSelector::parse(selector)?;
        self.reachable_ids()
            .into_iter()
            .find(|&id| selector.matches(self, id))
    }

    pub fn element_tag_name(&self, id: NodeId) -> Option<&str> {
        match &self.node(id)?.data {
            DomNodeData::Element { tag_name, .. } => Some(tag_name.as_str()),
            _ => None,
        }
    }

    pub fn attribute(&self, id: NodeId, name: &str) -> Option<&str> {
        match &self.node(id)?.data {
            DomNodeData::Element { attributes, .. } => attribute_value(attributes, name),
            _ => None,
        }
    }

    pub fn set_attribute(&mut self, id: NodeId, name: &str, value: &str) -> bool {
        if id >= self.nodes.len() || name.trim().is_empty() {
            return false;
        }
        let Some(node) = self.node_mut(id) else {
            return false;
        };
        let DomNodeData::Element { attributes, .. } = &mut node.data else {
            return false;
        };
        if let Some(attribute) = attributes
            .iter_mut()
            .find(|attribute| attribute.name.eq_ignore_ascii_case(name))
        {
            attribute.value = value.to_owned();
        } else {
            attributes.push(DomAttribute {
                namespace: None,
                prefix: None,
                name: name.to_owned(),
                value: value.to_owned(),
            });
        }
        true
    }

    pub fn remove_attribute(&mut self, id: NodeId, name: &str) -> bool {
        if id >= self.nodes.len() {
            return false;
        }
        let Some(node) = self.node_mut(id) else {
            return false;
        };
        let DomNodeData::Element { attributes, .. } = &mut node.data else {
            return false;
        };
        let before = attributes.len();
        attributes.retain(|attribute| !attribute.name.eq_ignore_ascii_case(name));
        attributes.len() != before
    }

    pub fn set_text_content(&mut self, id: NodeId, text: &str) -> bool {
        if id >= self.nodes.len() {
            return false;
        }

        if matches!(self.node(id).map(|node| &node.data), Some(DomNodeData::Text(_))) {
            if let Some(DomNode {
                data: DomNodeData::Text(current),
                ..
            }) = self.node_mut(id)
            {
                *current = text.to_owned();
                return true;
            }
        }

        let old_children = self
            .node(id)
            .map(|node| node.children.clone())
            .unwrap_or_default();
        for child in old_children {
            if let Some(node) = self.node_mut(child) {
                node.parent = None;
            }
        }

        let mut children = Vec::new();
        if !text.is_empty() {
            let text_id = self.nodes.len();
            self.nodes.push(DomNode {
                id: text_id,
                parent: Some(id),
                children: Vec::new(),
                data: DomNodeData::Text(text.to_owned()),
            });
            children.push(text_id);
        }

        if let Some(node) = self.node_mut(id) {
            node.children = children;
            true
        } else {
            false
        }
    }

    pub fn set_title(&mut self, title: &str) -> bool {
        if let Some(id) = self.find_first_element("title") {
            return self.set_text_content(id, title);
        }
        let Some(head) = self.find_first_element("head") else {
            return false;
        };
        let title_id = self.append_html_element(head, "title");
        self.set_text_content(title_id, title)
    }

    /// Creates a detached HTML element owned by the Nexus DOM. It becomes
    /// reachable only after `append_child` is called.
    pub fn create_element(&mut self, tag_name: &str) -> Option<NodeId> {
        let tag_name = tag_name.trim().to_ascii_lowercase();
        if tag_name.is_empty() || !tag_name.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-') {
            return None;
        }
        let id = self.nodes.len();
        self.nodes.push(DomNode {
            id,
            parent: None,
            children: Vec::new(),
            data: DomNodeData::Element {
                namespace: "http://www.w3.org/1999/xhtml".to_owned(),
                prefix: None,
                tag_name,
                attributes: Vec::new(),
            },
        });
        Some(id)
    }

    /// Appends an existing node under an element/document parent. Existing
    /// parentage is removed first, mirroring the DOM move semantics.
    pub fn append_child(&mut self, parent: NodeId, child: NodeId) -> bool {
        if parent >= self.nodes.len() || child >= self.nodes.len() || parent == child {
            return false;
        }
        if self.is_descendant_of(parent, child) {
            return false;
        }
        if !matches!(self.node(parent).map(|node| &node.data), Some(DomNodeData::Document | DomNodeData::Element { .. })) {
            return false;
        }
        if let Some(old_parent) = self.node(child).and_then(|node| node.parent) {
            if let Some(node) = self.node_mut(old_parent) {
                node.children.retain(|&id| id != child);
            }
        }
        if let Some(node) = self.node_mut(child) {
            node.parent = Some(parent);
        }
        if let Some(node) = self.node_mut(parent) {
            if !node.children.contains(&child) {
                node.children.push(child);
            }
            return true;
        }
        false
    }

    pub fn remove_child(&mut self, parent: NodeId, child: NodeId) -> bool {
        if parent >= self.nodes.len() || child >= self.nodes.len() {
            return false;
        }
        let before = self.node(parent).map_or(0, |node| node.children.len());
        if let Some(node) = self.node_mut(parent) {
            node.children.retain(|&id| id != child);
        }
        let changed = self.node(parent).is_some_and(|node| node.children.len() != before);
        if changed {
            if let Some(node) = self.node_mut(child) {
                node.parent = None;
            }
        }
        changed
    }

    pub fn parent_element(&self, id: NodeId) -> Option<NodeId> {
        let mut current = self.node(id)?.parent;
        while let Some(node_id) = current {
            if self.element_tag_name(node_id).is_some() {
                return Some(node_id);
            }
            current = self.node(node_id)?.parent;
        }
        None
    }

    pub fn closest_element(&self, id: NodeId) -> Option<NodeId> {
        if self.element_tag_name(id).is_some() {
            Some(id)
        } else {
            self.parent_element(id)
        }
    }

    pub fn closest_ancestor_tag(&self, id: NodeId, tag: &str) -> Option<NodeId> {
        let mut current = self.closest_element(id);
        while let Some(node_id) = current {
            if self.element_tag_name(node_id).is_some_and(|name| name.eq_ignore_ascii_case(tag)) {
                return Some(node_id);
            }
            current = self.node(node_id).and_then(|node| node.parent);
        }
        None
    }

    pub fn form_controls(&self, form: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        self.collect_form_controls(form, &mut out);
        out
    }

    fn collect_form_controls(&self, id: NodeId, out: &mut Vec<NodeId>) {
        let Some(node) = self.node(id) else { return; };
        for &child in &node.children {
            if let Some(tag) = self.element_tag_name(child) {
                if matches!(tag.to_ascii_lowercase().as_str(), "input" | "textarea" | "select" | "button") {
                    out.push(child);
                }
            }
            self.collect_form_controls(child, out);
        }
    }

    fn is_descendant_of(&self, possible_descendant: NodeId, possible_ancestor: NodeId) -> bool {
        let mut current = self.node(possible_descendant).and_then(|node| node.parent);
        for _ in 0..=self.nodes.len() {
            let Some(id) = current else { return false; };
            if id == possible_ancestor {
                return true;
            }
            current = self.node(id).and_then(|node| node.parent);
        }
        false
    }

    pub fn style_blocks(&self) -> Vec<String> {
        self.reachable_ids()
            .into_iter()
            .filter_map(|id| match self.node(id).map(|node| &node.data) {
                Some(DomNodeData::Element { tag_name, .. })
                    if tag_name.eq_ignore_ascii_case("style") => Some(self.text_content_raw(id)),
                _ => None,
            })
            .collect()
    }

    pub fn node_label(&self, id: NodeId) -> String {
        let Some(node) = self.node(id) else {
            return "<missing>".to_owned();
        };
        match &node.data {
            DomNodeData::Document => "#document".to_owned(),
            DomNodeData::Doctype { name, .. } => format!("!doctype {name}"),
            DomNodeData::Element { tag_name, .. } => {
                let mut label = tag_name.clone();
                if let Some(id_value) = self.attribute(id, "id") {
                    label.push('#');
                    label.push_str(id_value);
                }
                if let Some(classes) = self.attribute(id, "class") {
                    for class in classes.split_ascii_whitespace().take(2) {
                        label.push('.');
                        label.push_str(class);
                    }
                }
                label
            }
            DomNodeData::Text(text) => {
                format!("#text \"{}\"", truncate(&normalize_whitespace(text), 18))
            }
            DomNodeData::Comment(_) => "#comment".to_owned(),
            DomNodeData::ProcessingInstruction { target, .. } => format!("?{target}"),
        }
    }

    pub fn is_connected(&self, id: NodeId) -> bool {
        if id >= self.nodes.len() {
            return false;
        }
        let mut current = Some(id);
        for _ in 0..=self.nodes.len() {
            let Some(node_id) = current else {
                return false;
            };
            if node_id == self.root {
                return true;
            }
            current = self.node(node_id).and_then(|node| node.parent);
        }
        false
    }

    pub fn reachable_ids(&self) -> Vec<NodeId> {
        let mut out = Vec::new();
        self.collect_reachable(self.root, &mut out);
        out
    }

    fn collect_reachable(&self, id: NodeId, out: &mut Vec<NodeId>) {
        let Some(node) = self.node(id) else {
            return;
        };
        out.push(id);
        for &child in &node.children {
            self.collect_reachable(child, out);
        }
    }

    fn append_html_element(&mut self, parent: NodeId, tag_name: &str) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(DomNode {
            id,
            parent: Some(parent),
            children: Vec::new(),
            data: DomNodeData::Element {
                namespace: "http://www.w3.org/1999/xhtml".to_owned(),
                prefix: None,
                tag_name: tag_name.to_owned(),
                attributes: Vec::new(),
            },
        });
        if let Some(parent_node) = self.node_mut(parent) {
            parent_node.children.push(id);
        }
        id
    }

    fn base_href(&self) -> Option<String> {
        self.reachable_ids().into_iter().find_map(|id| match self.node(id).map(|node| &node.data) {
            Some(DomNodeData::Element {
                tag_name,
                attributes,
                ..
            }) if tag_name.eq_ignore_ascii_case("base") => {
                attribute_value(attributes, "href").map(str::to_owned)
            }
            _ => None,
        })
    }

    fn write_tree(&self, id: NodeId, depth: usize, max_depth: usize, out: &mut String) {
        if depth > max_depth {
            return;
        }
        let Some(node) = self.node(id) else {
            return;
        };
        if !self.is_connected(id) {
            return;
        }

        out.push_str(&"  ".repeat(depth));
        match &node.data {
            DomNodeData::Document => out.push_str("#document"),
            DomNodeData::Doctype { name, .. } => out.push_str(&format!("<!DOCTYPE {name}>")),
            DomNodeData::Element {
                tag_name,
                attributes,
                ..
            } => {
                out.push('<');
                out.push_str(tag_name);
                for attr in attributes.iter().take(4) {
                    out.push(' ');
                    out.push_str(&attr.name);
                    out.push_str("=\"");
                    out.push_str(&truncate(&attr.value, 40));
                    out.push('"');
                }
                if attributes.len() > 4 {
                    out.push_str(" …");
                }
                out.push('>');
            }
            DomNodeData::Text(text) => {
                out.push_str("#text \"");
                out.push_str(&truncate(&normalize_whitespace(text), 70));
                out.push('"');
            }
            DomNodeData::Comment(text) => {
                out.push_str("<!-- ");
                out.push_str(&truncate(text, 60));
                out.push_str(" -->");
            }
            DomNodeData::ProcessingInstruction { target, .. } => {
                out.push_str("<?");
                out.push_str(target);
                out.push_str("?>");
            }
        }
        out.push('\n');

        if depth == max_depth {
            if !node.children.is_empty() {
                out.push_str(&"  ".repeat(depth + 1));
                out.push_str("…\n");
            }
            return;
        }

        for &child in &node.children {
            self.write_tree(child, depth + 1, max_depth, out);
        }
    }
}

#[derive(Debug, Default)]
struct SimpleDomSelector {
    tag: Option<String>,
    id: Option<String>,
    classes: Vec<String>,
}

impl SimpleDomSelector {
    fn parse(input: &str) -> Option<Self> {
        let input = input.trim();
        if input.is_empty()
            || input
                .chars()
                .any(|ch| matches!(ch, ' ' | '>' | '+' | '~' | '[' | ':' | ','))
        {
            return None;
        }

        let mut selector = Self::default();
        let mut token = String::new();
        let mut mode = 't';

        let flush = |selector: &mut Self, mode: char, token: &mut String| {
            if token.is_empty() {
                return;
            }
            let value = std::mem::take(token);
            match mode {
                't' => selector.tag = Some(value),
                '#' => selector.id = Some(value),
                '.' => selector.classes.push(value),
                _ => {}
            }
        };

        for ch in input.chars() {
            if ch == '#' || ch == '.' {
                flush(&mut selector, mode, &mut token);
                mode = ch;
            } else if ch == '*' && mode == 't' && token.is_empty() {
                selector.tag = None;
            } else if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                token.push(ch);
            } else {
                return None;
            }
        }
        flush(&mut selector, mode, &mut token);

        (selector.tag.is_some() || selector.id.is_some() || !selector.classes.is_empty())
            .then_some(selector)
    }

    fn matches(&self, dom: &NexusDom, id: NodeId) -> bool {
        let Some(tag) = dom.element_tag_name(id) else {
            return false;
        };
        if self
            .tag
            .as_deref()
            .is_some_and(|expected| !tag.eq_ignore_ascii_case(expected))
        {
            return false;
        }
        if self
            .id
            .as_deref()
            .is_some_and(|expected| dom.attribute(id, "id") != Some(expected))
        {
            return false;
        }
        if !self.classes.is_empty() {
            let actual = dom
                .attribute(id, "class")
                .unwrap_or("")
                .split_ascii_whitespace()
                .collect::<Vec<_>>();
            if self
                .classes
                .iter()
                .any(|wanted| !actual.iter().any(|actual| *actual == wanted))
            {
                return false;
            }
        }
        true
    }
}

fn attribute_value<'a>(attributes: &'a [DomAttribute], name: &str) -> Option<&'a str> {
    attributes
        .iter()
        .find(|attr| attr.name.eq_ignore_ascii_case(name))
        .map(|attr| attr.value.as_str())
}

fn normalize_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate(input: &str, max_chars: usize) -> String {
    let mut chars = input.chars();
    let mut result: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        result.push('…');
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_html;

    #[test]
    fn mutation_updates_text_and_attribute() {
        let mut dom = parse_html(
            Url::parse("https://example.com/").unwrap(),
            r#"<p id="target">before</p>"#,
        );
        let id = dom.query_selector("#target").unwrap();
        assert!(dom.set_text_content(id, "after"));
        assert!(dom.set_attribute(id, "class", "changed"));
        assert_eq!(dom.text_content(id), "after");
        assert_eq!(dom.attribute(id, "class"), Some("changed"));
    }

    #[test]
    fn extracts_scripts_in_document_order() {
        let dom = parse_html(
            Url::parse("https://example.com/app/").unwrap(),
            r#"<script>one()</script><script src="two.js"></script><script type="module">three()</script>"#,
        );
        let scripts = dom.scripts();
        assert_eq!(scripts.len(), 3);
        assert_eq!(scripts[0].inline_code, "one()");
        assert_eq!(
            scripts[1].resolved_url.as_ref().unwrap().as_str(),
            "https://example.com/app/two.js"
        );
        assert!(scripts[2].is_module);
    }
}
