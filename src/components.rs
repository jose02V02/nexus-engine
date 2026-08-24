//! Executable Web Components model for Nexus Engine 1.02.
//!
//! The model implements registries, upgrade/reaction ordering, Shadow Root
//! ownership, inert template cloning and slot assignment. JavaScript class
//! callbacks and shadow-style encapsulation remain backend integration work.

use std::collections::{HashMap, VecDeque};

use url::Url;

use crate::dom::{DomNodeData, NexusDom, NodeId};
use crate::parser::parse_html;
use crate::web_platform::{CustomElementDefinition, CustomElementRegistry, ShadowRootMode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShadowNode {
    Element { tag_name: String, attributes: Vec<(String, String)>, children: Vec<ShadowNode> },
    Text(String),
    Comment(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateBlueprint { nodes: Vec<ShadowNode> }

impl TemplateBlueprint {
    #[must_use]
    pub fn parse(markup: &str) -> Self {
        let dom = parse_html(Url::parse("https://template.nexus/").expect("static URL"), markup);
        let container = dom.find_first_element("body").unwrap_or(dom.root());
        let nodes = dom.node(container).map_or_else(Vec::new, |node| {
            node.children.iter().filter_map(|id| clone_shadow_node(&dom, *id)).collect()
        });
        Self { nodes }
    }

    #[must_use]
    pub fn clone_content(&self) -> Vec<ShadowNode> { self.nodes.clone() }
}

fn clone_shadow_node(dom: &NexusDom, id: NodeId) -> Option<ShadowNode> {
    let node = dom.node(id)?;
    match &node.data {
        DomNodeData::Element { tag_name, attributes, .. } => Some(ShadowNode::Element {
            tag_name: tag_name.clone(),
            attributes: attributes.iter().map(|attribute| (attribute.name.clone(), attribute.value.clone())).collect(),
            children: node.children.iter().filter_map(|child| clone_shadow_node(dom, *child)).collect(),
        }),
        DomNodeData::Text(text) => Some(ShadowNode::Text(text.clone())),
        DomNodeData::Comment(text) => Some(ShadowNode::Comment(text.clone())),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowTree {
    pub host: NodeId,
    pub mode: ShadowRootMode,
    pub delegates_focus: bool,
    pub children: Vec<ShadowNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentError { HostNotElement, ShadowRootAlreadyAttached, InvalidDefinition(String) }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomElementReaction {
    Upgrade { node: NodeId, name: String },
    Connected { node: NodeId },
    Disconnected { node: NodeId },
    AttributeChanged { node: NodeId, name: String, old_value: Option<String>, new_value: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotAssignment { pub slot_name: String, pub nodes: Vec<NodeId> }

#[derive(Debug, Default)]
pub struct ComponentRuntime {
    registry: CustomElementRegistry,
    shadows: HashMap<NodeId, ShadowTree>,
    upgraded: HashMap<NodeId, String>,
    reactions: VecDeque<CustomElementReaction>,
}

impl ComponentRuntime {
    pub fn define(&mut self, dom: &NexusDom, definition: CustomElementDefinition) -> Result<usize, ComponentError> {
        let name = definition.name.to_ascii_lowercase();
        self.registry.define(definition).map_err(ComponentError::InvalidDefinition)?;
        let mut count = 0;
        for node in dom.nodes() {
            if matches!(&node.data, DomNodeData::Element { tag_name, .. } if tag_name.eq_ignore_ascii_case(&name))
                && self.upgraded.insert(node.id, name.clone()).is_none()
            {
                self.reactions.push_back(CustomElementReaction::Upgrade { node: node.id, name: name.clone() });
                self.reactions.push_back(CustomElementReaction::Connected { node: node.id });
                count += 1;
            }
        }
        Ok(count)
    }

    pub fn attach_shadow(
        &mut self, dom: &NexusDom, host: NodeId, mode: ShadowRootMode,
        delegates_focus: bool, children: Vec<ShadowNode>,
    ) -> Result<(), ComponentError> {
        if !matches!(dom.node(host).map(|node| &node.data), Some(DomNodeData::Element { .. })) {
            return Err(ComponentError::HostNotElement);
        }
        if self.shadows.contains_key(&host) { return Err(ComponentError::ShadowRootAlreadyAttached); }
        self.shadows.insert(host, ShadowTree { host, mode, delegates_focus, children });
        Ok(())
    }

    #[must_use]
    pub fn shadow_root(&self, host: NodeId) -> Option<&ShadowTree> {
        self.shadows.get(&host).filter(|root| root.mode == ShadowRootMode::Open)
    }

    #[must_use]
    pub fn shadow_root_internal(&self, host: NodeId) -> Option<&ShadowTree> { self.shadows.get(&host) }

    pub fn attribute_changed(
        &mut self, node: NodeId, name: &str, old_value: Option<String>, new_value: Option<String>,
    ) -> bool {
        let Some(tag_name) = self.upgraded.get(&node) else { return false };
        let Some(definition) = self.registry.get(tag_name) else { return false };
        if !definition.observed_attributes.iter().any(|attribute| attribute.eq_ignore_ascii_case(name)) { return false; }
        self.reactions.push_back(CustomElementReaction::AttributeChanged {
            node, name: name.to_ascii_lowercase(), old_value, new_value,
        });
        true
    }

    pub fn disconnected(&mut self, node: NodeId) {
        if self.upgraded.contains_key(&node) { self.reactions.push_back(CustomElementReaction::Disconnected { node }); }
    }

    pub fn drain_reactions(&mut self) -> Vec<CustomElementReaction> { self.reactions.drain(..).collect() }

    #[must_use]
    pub fn assign_slots(dom: &NexusDom, host: NodeId, declared_slots: &[String]) -> Vec<SlotAssignment> {
        let mut assignments = declared_slots.iter().map(|name| SlotAssignment { slot_name: name.clone(), nodes: Vec::new() }).collect::<Vec<_>>();
        if !assignments.iter().any(|slot| slot.slot_name.is_empty()) {
            assignments.push(SlotAssignment { slot_name: String::new(), nodes: Vec::new() });
        }
        let Some(host_node) = dom.node(host) else { return assignments };
        for child in &host_node.children {
            let requested = dom.attribute(*child, "slot").unwrap_or("");
            let index = assignments.iter().position(|slot| slot.slot_name == requested)
                .or_else(|| assignments.iter().position(|slot| slot.slot_name.is_empty()));
            if let Some(index) = index { assignments[index].nodes.push(*child); }
        }
        assignments
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dom(markup: &str) -> NexusDom { parse_html(Url::parse("https://app.test/").unwrap(), markup) }

    #[test]
    fn templates_clone_inert_subtrees() {
        let template = TemplateBlueprint::parse("<section><h2>Card</h2><slot></slot></section>");
        let first = template.clone_content();
        let second = template.clone_content();
        assert_eq!(first, second);
        assert!(!first.is_empty());
    }

    #[test]
    fn defining_a_custom_element_upgrades_existing_candidates_in_order() {
        let dom = dom("<nexus-card></nexus-card><nexus-card></nexus-card>");
        let mut runtime = ComponentRuntime::default();
        assert_eq!(runtime.define(&dom, CustomElementDefinition { name: "nexus-card".to_owned(), observed_attributes: vec!["title".to_owned()] }).unwrap(), 2);
        let reactions = runtime.drain_reactions();
        assert_eq!(reactions.len(), 4);
        assert!(matches!(reactions[0], CustomElementReaction::Upgrade { .. }));
        assert!(matches!(reactions[1], CustomElementReaction::Connected { .. }));
    }

    #[test]
    fn a_host_accepts_only_one_shadow_root() {
        let dom = dom("<div id='host'></div>");
        let host = dom.find_element_by_id("host").unwrap();
        let mut runtime = ComponentRuntime::default();
        runtime.attach_shadow(&dom, host, ShadowRootMode::Open, false, Vec::new()).unwrap();
        assert_eq!(runtime.attach_shadow(&dom, host, ShadowRootMode::Open, false, Vec::new()), Err(ComponentError::ShadowRootAlreadyAttached));
    }

    #[test]
    fn closed_roots_are_hidden_from_public_lookup() {
        let dom = dom("<div id='host'></div>");
        let host = dom.find_element_by_id("host").unwrap();
        let mut runtime = ComponentRuntime::default();
        runtime.attach_shadow(&dom, host, ShadowRootMode::Closed, true, Vec::new()).unwrap();
        assert!(runtime.shadow_root(host).is_none());
        assert!(runtime.shadow_root_internal(host).is_some());
    }

    #[test]
    fn named_and_default_slots_receive_light_dom_children() {
        let dom = dom("<x-panel id='host'><h2 slot='title'>Title</h2><p>Body</p></x-panel>");
        let host = dom.find_element_by_id("host").unwrap();
        let assignments = ComponentRuntime::assign_slots(&dom, host, &["title".to_owned(), String::new()]);
        assert_eq!(assignments[0].nodes.len(), 1);
        assert_eq!(assignments[1].nodes.len(), 1);
    }

    #[test]
    fn only_observed_attributes_enqueue_reactions() {
        let dom = dom("<nexus-card id='card'></nexus-card>");
        let card = dom.find_element_by_id("card").unwrap();
        let mut runtime = ComponentRuntime::default();
        runtime.define(&dom, CustomElementDefinition { name: "nexus-card".to_owned(), observed_attributes: vec!["title".to_owned()] }).unwrap();
        assert!(runtime.attribute_changed(card, "title", None, Some("Hello".to_owned())));
        assert!(!runtime.attribute_changed(card, "class", None, Some("large".to_owned())));
    }
}
