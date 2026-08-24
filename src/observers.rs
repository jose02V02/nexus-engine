//! DOM and geometry observers for Nexus Engine 1.02.

use std::collections::{HashMap, VecDeque};

use crate::dom::{NexusDom, NodeId};
use crate::layout::{LayoutBox, LayoutTree};

pub type ObserverId = u64;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MutationObserverOptions {
    pub attributes: bool,
    pub child_list: bool,
    pub character_data: bool,
    pub subtree: bool,
    pub attribute_old_value: bool,
    pub character_data_old_value: bool,
    pub attribute_filter: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MutationRecordKind { Attributes, ChildList, CharacterData }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutationRecord {
    pub target: NodeId,
    pub kind: MutationRecordKind,
    pub attribute_name: Option<String>,
    pub old_value: Option<String>,
    pub added_nodes: Vec<NodeId>,
    pub removed_nodes: Vec<NodeId>,
}

#[derive(Debug, Clone)]
struct MutationRegistration { target: NodeId, options: MutationObserverOptions }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IntersectionEntry { pub target: NodeId, pub ratio: f32, pub is_intersecting: bool }

#[derive(Debug, Clone)]
struct IntersectionRegistration { target: NodeId, thresholds: Vec<f32>, last_ratio: Option<f32> }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResizeEntry { pub target: NodeId, pub width: f32, pub height: f32 }

#[derive(Debug, Default)]
pub struct ObserverRuntime {
    next_id: ObserverId,
    mutations: HashMap<ObserverId, MutationRegistration>,
    mutation_records: HashMap<ObserverId, VecDeque<MutationRecord>>,
    intersections: HashMap<ObserverId, IntersectionRegistration>,
    intersection_records: HashMap<ObserverId, VecDeque<IntersectionEntry>>,
    resizes: HashMap<ObserverId, NodeId>,
    resize_records: HashMap<ObserverId, VecDeque<ResizeEntry>>,
    last_sizes: HashMap<ObserverId, (f32, f32)>,
}

impl ObserverRuntime {
    fn allocate(&mut self) -> ObserverId {
        self.next_id = self.next_id.saturating_add(1).max(1);
        self.next_id
    }

    pub fn observe_mutations(&mut self, target: NodeId, options: MutationObserverOptions) -> ObserverId {
        let id = self.allocate();
        self.mutations.insert(id, MutationRegistration { target, options });
        id
    }

    pub fn observe_intersection(&mut self, target: NodeId, mut thresholds: Vec<f32>) -> ObserverId {
        thresholds.retain(|value| value.is_finite());
        thresholds.iter_mut().for_each(|value| *value = value.clamp(0.0, 1.0));
        thresholds.sort_by(f32::total_cmp);
        thresholds.dedup_by(|a, b| (*a - *b).abs() < f32::EPSILON);
        if thresholds.is_empty() { thresholds.push(0.0); }
        let id = self.allocate();
        self.intersections.insert(id, IntersectionRegistration { target, thresholds, last_ratio: None });
        id
    }

    pub fn observe_resize(&mut self, target: NodeId) -> ObserverId {
        let id = self.allocate();
        self.resizes.insert(id, target);
        id
    }

    pub fn disconnect(&mut self, id: ObserverId) {
        self.mutations.remove(&id); self.mutation_records.remove(&id);
        self.intersections.remove(&id); self.intersection_records.remove(&id);
        self.resizes.remove(&id); self.resize_records.remove(&id);
    }

    fn notify_mutation(&mut self, dom: &NexusDom, record: MutationRecord) {
        for (&id, registration) in &self.mutations {
            let in_scope = record.target == registration.target
                || (registration.options.subtree && is_descendant(dom, record.target, registration.target));
            if !in_scope || !mutation_enabled(&registration.options, &record) { continue; }
            let mut delivered = record.clone();
            if delivered.kind == MutationRecordKind::Attributes && !registration.options.attribute_old_value { delivered.old_value = None; }
            if delivered.kind == MutationRecordKind::CharacterData && !registration.options.character_data_old_value { delivered.old_value = None; }
            self.mutation_records.entry(id).or_default().push_back(delivered);
        }
    }

    pub fn update_geometry(&mut self, layout: &LayoutTree, scroll_y: f32) {
        let viewport = (0.0, scroll_y, layout.viewport.width, layout.viewport.height);
        for (&id, registration) in &mut self.intersections {
            let ratio = layout.box_for(registration.target).map_or(0.0, |rect| intersection_ratio(rect, viewport));
            let crossed = registration.last_ratio.is_none_or(|old| {
                (old <= 0.0 && ratio > 0.0) || (old > 0.0 && ratio <= 0.0)
                    || registration.thresholds.iter().any(|threshold|
                        (old < *threshold && ratio >= *threshold) || (old >= *threshold && ratio < *threshold))
            });
            if crossed {
                self.intersection_records.entry(id).or_default().push_back(IntersectionEntry {
                    target: registration.target, ratio, is_intersecting: ratio > 0.0,
                });
            }
            registration.last_ratio = Some(ratio);
        }
        for (&id, &target) in &self.resizes {
            let Some(rect) = layout.box_for(target) else { continue };
            let size = (rect.width, rect.height);
            let changed = self.last_sizes.get(&id).is_none_or(|old| (old.0 - size.0).abs() > 0.01 || (old.1 - size.1).abs() > 0.01);
            if changed {
                self.resize_records.entry(id).or_default().push_back(ResizeEntry { target, width: size.0, height: size.1 });
            }
            self.last_sizes.insert(id, size);
        }
    }

    pub fn take_mutations(&mut self, id: ObserverId) -> Vec<MutationRecord> {
        self.mutation_records.entry(id).or_default().drain(..).collect()
    }
    pub fn take_intersections(&mut self, id: ObserverId) -> Vec<IntersectionEntry> {
        self.intersection_records.entry(id).or_default().drain(..).collect()
    }
    pub fn take_resizes(&mut self, id: ObserverId) -> Vec<ResizeEntry> {
        self.resize_records.entry(id).or_default().drain(..).collect()
    }
}

fn mutation_enabled(options: &MutationObserverOptions, record: &MutationRecord) -> bool {
    match record.kind {
        MutationRecordKind::Attributes => options.attributes && record.attribute_name.as_ref().is_none_or(|name|
            options.attribute_filter.is_empty() || options.attribute_filter.iter().any(|candidate| candidate.eq_ignore_ascii_case(name))),
        MutationRecordKind::ChildList => options.child_list,
        MutationRecordKind::CharacterData => options.character_data,
    }
}

fn is_descendant(dom: &NexusDom, node: NodeId, ancestor: NodeId) -> bool {
    let mut current = dom.node(node).and_then(|item| item.parent);
    while let Some(id) = current {
        if id == ancestor { return true; }
        current = dom.node(id).and_then(|item| item.parent);
    }
    false
}

fn intersection_ratio(rect: &LayoutBox, viewport: (f32, f32, f32, f32)) -> f32 {
    let (vx, vy, vw, vh) = viewport;
    let left = rect.x.max(vx); let top = rect.y.max(vy);
    let right = (rect.x + rect.width).min(vx + vw); let bottom = (rect.y + rect.height).min(vy + vh);
    let intersection = (right - left).max(0.0) * (bottom - top).max(0.0);
    let area = rect.width.max(0.0) * rect.height.max(0.0);
    if area <= f32::EPSILON { 0.0 } else { (intersection / area).clamp(0.0, 1.0) }
}

pub struct ObservedDom<'a> { dom: &'a mut NexusDom, observers: &'a mut ObserverRuntime }

impl<'a> ObservedDom<'a> {
    pub fn new(dom: &'a mut NexusDom, observers: &'a mut ObserverRuntime) -> Self { Self { dom, observers } }
    pub fn dom(&self) -> &NexusDom { self.dom }

    pub fn set_attribute(&mut self, target: NodeId, name: &str, value: &str) -> bool {
        let old = self.dom.attribute(target, name).map(str::to_owned);
        if !self.dom.set_attribute(target, name, value) { return false; }
        self.observers.notify_mutation(self.dom, MutationRecord { target, kind: MutationRecordKind::Attributes,
            attribute_name: Some(name.to_ascii_lowercase()), old_value: old, added_nodes: Vec::new(), removed_nodes: Vec::new() });
        true
    }

    pub fn set_text_content(&mut self, target: NodeId, text: &str) -> bool {
        let old = self.dom.text_content_raw(target);
        if !self.dom.set_text_content(target, text) { return false; }
        self.observers.notify_mutation(self.dom, MutationRecord { target, kind: MutationRecordKind::CharacterData,
            attribute_name: None, old_value: Some(old), added_nodes: Vec::new(), removed_nodes: Vec::new() });
        true
    }

    pub fn append_child(&mut self, parent: NodeId, child: NodeId) -> bool {
        if !self.dom.append_child(parent, child) { return false; }
        self.observers.notify_mutation(self.dom, MutationRecord { target: parent, kind: MutationRecordKind::ChildList,
            attribute_name: None, old_value: None, added_nodes: vec![child], removed_nodes: Vec::new() });
        true
    }

    pub fn remove_child(&mut self, parent: NodeId, child: NodeId) -> bool {
        if !self.dom.remove_child(parent, child) { return false; }
        self.observers.notify_mutation(self.dom, MutationRecord { target: parent, kind: MutationRecordKind::ChildList,
            attribute_name: None, old_value: None, added_nodes: Vec::new(), removed_nodes: vec![child] });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{compute_layout, compute_styles_for_viewport, MediaEnvironment, Viewport};
    use crate::parser::parse_html;
    use url::Url;

    fn dom(markup: &str) -> NexusDom { parse_html(Url::parse("https://app.test/").unwrap(), markup) }

    #[test]
    fn attribute_filter_and_old_value_are_respected() {
        let mut dom = dom("<div id='host' class='old'></div>"); let host = dom.find_element_by_id("host").unwrap();
        let mut runtime = ObserverRuntime::default();
        let id = runtime.observe_mutations(host, MutationObserverOptions { attributes: true, attribute_old_value: true, attribute_filter: vec!["class".to_owned()], ..Default::default() });
        let mut observed = ObservedDom::new(&mut dom, &mut runtime);
        observed.set_attribute(host, "title", "ignored"); observed.set_attribute(host, "class", "new");
        let records = runtime.take_mutations(id); assert_eq!(records.len(), 1); assert_eq!(records[0].old_value.as_deref(), Some("old"));
    }

    #[test]
    fn subtree_child_list_observation_receives_nested_changes() {
        let mut dom = dom("<main id='root'><section id='nested'></section></main>");
        let root = dom.find_element_by_id("root").unwrap(); let nested = dom.find_element_by_id("nested").unwrap(); let child = dom.create_element("p").unwrap();
        let mut runtime = ObserverRuntime::default();
        let id = runtime.observe_mutations(root, MutationObserverOptions { child_list: true, subtree: true, ..Default::default() });
        ObservedDom::new(&mut dom, &mut runtime).append_child(nested, child);
        assert_eq!(runtime.take_mutations(id)[0].added_nodes, vec![child]);
    }

    #[test]
    fn disconnected_observers_receive_no_more_records() {
        let mut dom = dom("<div id='x'></div>"); let target = dom.find_element_by_id("x").unwrap();
        let mut runtime = ObserverRuntime::default(); let id = runtime.observe_mutations(target, MutationObserverOptions { attributes: true, ..Default::default() }); runtime.disconnect(id);
        ObservedDom::new(&mut dom, &mut runtime).set_attribute(target, "class", "x"); assert!(runtime.take_mutations(id).is_empty());
    }

    #[test]
    fn resize_records_are_deduplicated_until_geometry_changes() {
        let dom = dom("<div id='x' style='width:100px;height:40px'></div>"); let target = dom.find_element_by_id("x").unwrap();
        let styles = compute_styles_for_viewport(&dom, MediaEnvironment { width: 300.0, height: 200.0 });
        let layout = compute_layout(&dom, &styles, Viewport { width: 300.0, height: 200.0 }).unwrap();
        let mut runtime = ObserverRuntime::default(); let id = runtime.observe_resize(target);
        runtime.update_geometry(&layout, 0.0); runtime.update_geometry(&layout, 0.0); assert_eq!(runtime.take_resizes(id).len(), 1);
    }

    #[test]
    fn intersection_thresholds_fire_when_scroll_crosses_them() {
        let dom = dom("<div style='height:300px'></div><div id='x' style='width:100px;height:100px'></div>"); let target = dom.find_element_by_id("x").unwrap();
        let styles = compute_styles_for_viewport(&dom, MediaEnvironment { width: 200.0, height: 200.0 });
        let layout = compute_layout(&dom, &styles, Viewport { width: 200.0, height: 200.0 }).unwrap();
        let mut runtime = ObserverRuntime::default(); let id = runtime.observe_intersection(target, vec![0.5]);
        runtime.update_geometry(&layout, 0.0); let first = runtime.take_intersections(id); assert_eq!(first.len(), 1);
        runtime.update_geometry(&layout, 250.0); assert!(!runtime.take_intersections(id).is_empty());
    }
}
