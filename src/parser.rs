//! HTML5 parsing via Servo's `html5ever`, converted immediately into Nexus DOM.

use html5ever::tendril::TendrilSink;
use html5ever::parse_document;
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use url::Url;

use crate::dom::{DomAttribute, DomNode, DomNodeData, NexusDom, NodeId};

pub fn parse_html(document_url: Url, html: &str) -> NexusDom {
    let rcdom: RcDom = parse_document(RcDom::default(), Default::default()).one(html.to_owned());
    let parse_errors = rcdom
        .errors
        .borrow()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    let mut builder = DomBuilder::default();
    let root = builder.import_node(&rcdom.document, None);
    NexusDom::new(document_url, root, builder.nodes, parse_errors)
}

#[derive(Default)]
struct DomBuilder {
    nodes: Vec<DomNode>,
}

impl DomBuilder {
    fn import_node(&mut self, handle: &Handle, parent: Option<NodeId>) -> NodeId {
        let id = self.nodes.len();
        let data = convert_node_data(&handle.data);
        self.nodes.push(DomNode {
            id,
            parent,
            children: Vec::new(),
            data,
        });

        let children = handle.children.borrow().clone();
        for child in children {
            let child_id = self.import_node(&child, Some(id));
            self.nodes[id].children.push(child_id);
        }

        // <template> has a separate template-contents document in html5ever.
        if let NodeData::Element {
            template_contents, ..
        } = &handle.data
        {
            if let Some(template_root) = template_contents.borrow().as_ref() {
                let child_id = self.import_node(template_root, Some(id));
                self.nodes[id].children.push(child_id);
            }
        }

        id
    }
}

fn convert_node_data(data: &NodeData) -> DomNodeData {
    match data {
        NodeData::Document => DomNodeData::Document,
        NodeData::Doctype {
            name,
            public_id,
            system_id,
        } => DomNodeData::Doctype {
            name: name.to_string(),
            public_id: public_id.to_string(),
            system_id: system_id.to_string(),
        },
        NodeData::Text { contents } => DomNodeData::Text(contents.borrow().to_string()),
        NodeData::Comment { contents } => DomNodeData::Comment(contents.to_string()),
        NodeData::Element { name, attrs, .. } => {
            let attributes = attrs
                .borrow()
                .iter()
                .map(|attr| DomAttribute {
                    namespace: namespace_to_option(attr.name.ns.as_ref()),
                    prefix: attr.name.prefix.as_ref().map(ToString::to_string),
                    name: attr.name.local.to_string(),
                    value: attr.value.to_string(),
                })
                .collect();

            DomNodeData::Element {
                namespace: name.ns.to_string(),
                prefix: name.prefix.as_ref().map(ToString::to_string),
                tag_name: name.local.to_string(),
                attributes,
            }
        }
        NodeData::ProcessingInstruction { target, contents } => {
            DomNodeData::ProcessingInstruction {
                target: target.to_string(),
                contents: contents.to_string(),
            }
        }
    }
}

fn namespace_to_option(namespace: &str) -> Option<String> {
    if namespace.is_empty() {
        None
    } else {
        Some(namespace.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_nexus_dom() {
        let url = Url::parse("https://example.com/").unwrap();
        let dom = parse_html(url, "<title>Nexus</title><p>Hello <b>world</b></p>");
        assert_eq!(dom.title().as_deref(), Some("Nexus"));
        assert!(dom.body_text().contains("Hello world"));
        assert!(dom.nodes().len() >= 6);
    }

    #[test]
    fn extracts_and_resolves_links() {
        let url = Url::parse("https://example.com/docs/index.html").unwrap();
        let dom = parse_html(url, r#"<a href="../about">About</a>"#);
        let links = dom.links();
        assert_eq!(links.len(), 1);
        assert_eq!(
            links[0].resolved_url.as_ref().unwrap().as_str(),
            "https://example.com/about"
        );
    }
}
