//! `CapabilitySet` — a hash-set wrapper with utility methods.
use std::collections::HashSet;
use serde::{Deserialize, Serialize};
use crate::Capability;

/// A set of capabilities that defines what a subject is allowed to do.
///
/// The presence of the `Admin` capability causes **all** `check` and
/// `check_path` calls to return `true`, regardless of the requested capability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilitySet(HashSet<Capability>);

impl CapabilitySet {
    /// Create an empty capability set.
    pub fn new() -> Self {
        Self(HashSet::new())
    }

    /// Add a capability to the set.
    pub fn grant(&mut self, capability: Capability) {
        self.0.insert(capability);
    }

    /// Remove a capability from the set.
    pub fn revoke(&mut self, capability: &Capability) {
        self.0.remove(capability);
    }

    /// Check whether the given capability is present.
    ///
    /// Returns `true` if `Admin` is in the set *or* if the specific
    /// capability is present.
    pub fn check(&self, capability: &Capability) -> bool {
        self.0.contains(&Capability::Admin) || self.0.contains(capability)
    }

    /// Check a path-based capability using a constructor function.
    ///
    /// The `capability_fn` is called with `"*"` and with `path`; the
    /// method returns `true` if either resulting capability is in the set,
    /// or if `Admin` is present.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let set = CapabilitySet::new();
    /// // Grant read access to any path:
    /// set.grant(Capability::FileRead("*".into()));
    /// assert!(set.check_path(Capability::FileRead, "/etc/passwd"));
    /// ```
    pub fn check_path(
        &self,
        capability_fn: impl Fn(String) -> Capability,
        path: &str,
    ) -> bool {
        self.0.contains(&Capability::Admin)
            || self.0.contains(&capability_fn("*".to_string()))
            || self.0.contains(&capability_fn(path.to_string()))
    }

    /// Returns `true` if the set contains no capabilities (other than
    /// `Admin`, if present).
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Add all capabilities from the given iterator.
    pub fn extend(&mut self, iter: impl IntoIterator<Item = Capability>) {
        self.0.extend(iter);
    }
}

impl Default for CapabilitySet {
    fn default() -> Self {
        Self::new()
    }
}

impl From<HashSet<Capability>> for CapabilitySet {
    fn from(set: HashSet<Capability>) -> Self {
        Self(set)
    }
}

impl IntoIterator for CapabilitySet {
    type Item = Capability;
    type IntoIter = std::collections::hash_set::IntoIter<Capability>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a CapabilitySet {
    type Item = &'a Capability;
    type IntoIter = std::collections::hash_set::Iter<'a, Capability>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}
