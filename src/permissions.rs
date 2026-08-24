//! Minimal origin-scoped permission store for Nexus Engine 1.02.

use std::collections::HashMap;

use crate::origin::Origin;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionState {
    Prompt,
    Granted,
    Denied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PermissionKind {
    Geolocation,
    Notifications,
    Camera,
    Microphone,
    ClipboardRead,
    ClipboardWrite,
}

#[derive(Debug, Default)]
pub struct PermissionStore {
    entries: HashMap<(String, PermissionKind), PermissionState>,
}

impl PermissionStore {
    #[must_use]
    pub fn get(&self, origin: &Origin, kind: PermissionKind) -> PermissionState {
        self.entries
            .get(&(origin.serialize(), kind))
            .copied()
            .unwrap_or(PermissionState::Prompt)
    }

    pub fn set(&mut self, origin: &Origin, kind: PermissionKind, state: PermissionState) {
        if !matches!(origin, Origin::Opaque) {
            self.entries.insert((origin.serialize(), kind), state);
        }
    }

    pub fn clear_origin(&mut self, origin: &Origin) {
        let key = origin.serialize();
        self.entries.retain(|(stored, _), _| stored != &key);
    }

    pub fn clear_all(&mut self) {
        self.entries.clear();
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}
