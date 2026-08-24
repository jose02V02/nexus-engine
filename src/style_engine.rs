//! Pluggable CSS engine boundary.
//!
//! Nexus Engine 1.02 keeps the style map Nexus-owned while adding viewport-aware
//! media-query evaluation. A future Stylo adapter can implement the same trait.

use crate::css::{compute_styles, compute_styles_for_viewport, MediaEnvironment, StyleMap};
use crate::dom::NexusDom;

pub trait StyleEngine: Send + Sync {
    fn name(&self) -> &'static str;
    fn compute(&self, dom: &NexusDom) -> StyleMap;

    fn compute_for_viewport(&self, dom: &NexusDom, width: f32, height: f32) -> StyleMap {
        let _ = (width, height);
        self.compute(dom)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NexusStyleEngine;

impl StyleEngine for NexusStyleEngine {
    fn name(&self) -> &'static str {
        "NexusStyleEngine/cssparser"
    }

    fn compute(&self, dom: &NexusDom) -> StyleMap {
        compute_styles(dom)
    }

    fn compute_for_viewport(&self, dom: &NexusDom, width: f32, height: f32) -> StyleMap {
        compute_styles_for_viewport(dom, MediaEnvironment { width, height })
    }
}
