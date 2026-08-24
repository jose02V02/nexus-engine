//! HTML form-control model for Nexus Engine 1.02.
//!
//! The browser core exposes a platform-neutral descriptor to Android/Desktop
//! frontends. Native UI decides how to edit a control; Nexus remains the
//! source of truth for DOM state, validation and form submission.

use std::path::PathBuf;

use crate::dom::{NexusDom, NodeId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectOption {
    pub index: usize,
    pub value: String,
    pub label: String,
    pub selected: bool,
    pub disabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormControlDescriptor {
    pub node_id: NodeId,
    pub tag: String,
    pub input_type: String,
    pub name: String,
    pub value: String,
    pub placeholder: String,
    pub autocomplete: String,
    pub accept: String,
    pub required: bool,
    pub disabled: bool,
    pub readonly: bool,
    pub checked: bool,
    pub multiple: bool,
    pub min: String,
    pub max: String,
    pub step: String,
    pub options: Vec<SelectOption>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedFile {
    pub path: PathBuf,
    pub name: String,
    pub mime_type: String,
    pub size: u64,
}

pub fn describe_control(dom: &NexusDom, node: NodeId) -> Option<FormControlDescriptor> {
    let tag = dom.element_tag_name(node)?.to_ascii_lowercase();
    if !matches!(tag.as_str(), "input" | "textarea" | "select" | "button") {
        return None;
    }
    let input_type = if tag == "input" {
        dom.attribute(node, "type").unwrap_or("text").to_ascii_lowercase()
    } else {
        tag.clone()
    };
    let value = match tag.as_str() {
        "textarea" => dom.text_content_raw(node),
        "select" => selected_option_values(dom, node).first().cloned().unwrap_or_default(),
        _ => dom.attribute(node, "value").unwrap_or("").to_owned(),
    };
    Some(FormControlDescriptor {
        node_id: node,
        tag: tag.clone(),
        input_type,
        name: dom.attribute(node, "name").unwrap_or("").to_owned(),
        value,
        placeholder: dom.attribute(node, "placeholder").unwrap_or("").to_owned(),
        autocomplete: dom.attribute(node, "autocomplete").unwrap_or("").to_owned(),
        accept: dom.attribute(node, "accept").unwrap_or("").to_owned(),
        required: dom.attribute(node, "required").is_some(),
        disabled: dom.attribute(node, "disabled").is_some(),
        readonly: dom.attribute(node, "readonly").is_some(),
        checked: dom.attribute(node, "checked").is_some(),
        multiple: dom.attribute(node, "multiple").is_some(),
        min: dom.attribute(node, "min").unwrap_or("").to_owned(),
        max: dom.attribute(node, "max").unwrap_or("").to_owned(),
        step: dom.attribute(node, "step").unwrap_or("").to_owned(),
        options: if tag == "select" { select_options(dom, node) } else { Vec::new() },
    })
}

pub fn select_options(dom: &NexusDom, select: NodeId) -> Vec<SelectOption> {
    let mut option_ids = Vec::new();
    collect_options(dom, select, &mut option_ids);
    let has_explicit_selection = option_ids.iter().any(|&id| dom.attribute(id, "selected").is_some());
    option_ids
        .into_iter()
        .enumerate()
        .map(|(index, id)| {
            let label = dom.attribute(id, "label")
                .map(str::to_owned)
                .unwrap_or_else(|| dom.text_content_raw(id).trim().to_owned());
            let value = dom.attribute(id, "value").map(str::to_owned).unwrap_or_else(|| label.clone());
            SelectOption {
                index,
                value,
                label,
                selected: dom.attribute(id, "selected").is_some() || (!has_explicit_selection && index == 0),
                disabled: dom.attribute(id, "disabled").is_some(),
            }
        })
        .collect()
}

pub fn selected_option_values(dom: &NexusDom, select: NodeId) -> Vec<String> {
    select_options(dom, select)
        .into_iter()
        .filter(|option| option.selected && !option.disabled)
        .map(|option| option.value)
        .collect()
}

pub fn set_select_indices(dom: &mut NexusDom, select: NodeId, indices: &[usize]) -> bool {
    let multiple = dom.attribute(select, "multiple").is_some();
    let mut option_ids = Vec::new();
    collect_options(dom, select, &mut option_ids);
    let selected = if multiple { indices.to_vec() } else { indices.first().copied().into_iter().collect() };
    let mut changed = false;
    for (index, id) in option_ids.into_iter().enumerate() {
        let should_select = selected.contains(&index) && dom.attribute(id, "disabled").is_none();
        let is_selected = dom.attribute(id, "selected").is_some();
        if should_select != is_selected {
            if should_select { dom.set_attribute(id, "selected", ""); }
            else { dom.remove_attribute(id, "selected"); }
            changed = true;
        }
    }
    changed
}

fn collect_options(dom: &NexusDom, id: NodeId, output: &mut Vec<NodeId>) {
    let Some(node) = dom.node(id) else { return; };
    for &child in &node.children {
        if dom.element_tag_name(child).is_some_and(|tag| tag.eq_ignore_ascii_case("option")) {
            output.push(child);
        } else {
            collect_options(dom, child, output);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_html;
    use url::Url;

    #[test]
    fn describes_mobile_input_metadata() {
        let html = r#"<form><input id='email' type='email' name='mail' required autocomplete='email' placeholder='you@example.com'></form>"#;
        let dom = parse_html(Url::parse("https://nexus.test/").unwrap(), html);
        let id = dom.find_element_by_id("email").unwrap();
        let info = describe_control(&dom, id).unwrap();
        assert_eq!(info.input_type, "email");
        assert_eq!(info.name, "mail");
        assert!(info.required);
        assert_eq!(info.autocomplete, "email");
    }

    #[test]
    fn select_model_supports_single_selection() {
        let html = r#"<select id='s'><option value='a'>A</option><option value='b' selected>B</option></select>"#;
        let mut dom = parse_html(Url::parse("https://nexus.test/").unwrap(), html);
        let id = dom.find_element_by_id("s").unwrap();
        assert_eq!(selected_option_values(&dom, id), vec!["b"]);
        assert!(set_select_indices(&mut dom, id, &[0]));
        assert_eq!(selected_option_values(&dom, id), vec!["a"]);
    }
}
