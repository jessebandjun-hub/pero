//! Saved catalog order. Listing is this sequence applied to the apps installed now.

use super::AppName;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// User-controlled order of the App catalog. `names` is the last accepted
/// reorder, not a live copy of the installed set.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CatalogOrder {
    names: Vec<AppName>,
}

impl CatalogOrder {
    pub(super) fn empty() -> Self {
        Self { names: Vec::new() }
    }

    /// Saved names that are still installed, in saved order, then every other
    /// installed name in the order of `installed`. The first occurrence wins.
    pub(super) fn applied_to(&self, installed: &[AppName]) -> Vec<AppName> {
        let installed_set: BTreeSet<&AppName> = installed.iter().collect();
        let mut seen = BTreeSet::new();
        let mut order = Vec::with_capacity(installed.len());
        for name in &self.names {
            if installed_set.contains(name) && seen.insert(name) {
                order.push(name.clone());
            }
        }
        for name in installed {
            if seen.insert(name) {
                order.push(name.clone());
            }
        }
        order
    }

    /// `(installed, requested) -> next order`. The caller keeps its current
    /// order when this returns an error. `installed` is a set.
    pub(super) fn reorder(installed: &[AppName], requested: &[AppName]) -> Result<Self, String> {
        let installed_set: BTreeSet<&AppName> = installed.iter().collect();
        let mut seen = BTreeSet::new();
        for name in requested {
            if !installed_set.contains(name) {
                return Err("Catalog order names an App that is not installed".into());
            }
            if !seen.insert(name) {
                return Err("Catalog order lists an App more than once".into());
            }
        }
        if seen.len() != installed_set.len() {
            return Err("Catalog order must list each installed App once".into());
        }
        Ok(Self {
            names: requested.to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &str) -> Vec<AppName> {
        list.split(',')
            .filter(|name| !name.is_empty())
            .map(|name| AppName::try_from(name.to_string()).unwrap())
            .collect()
    }

    #[test]
    fn listing_keeps_saved_order_then_appends_the_remaining_installed_names() {
        for (saved, installed, expected) in [
            ("", "alpha,beta,gamma", "alpha,beta,gamma"),
            ("gamma,alpha,beta", "alpha,beta,gamma", "gamma,alpha,beta"),
            ("gamma,alpha", "alpha,beta,gamma", "gamma,alpha,beta"),
            (
                "beta,missing,alpha,beta",
                "alpha,beta,gamma",
                "beta,alpha,gamma",
            ),
            ("missing,gone", "alpha,beta", "alpha,beta"),
            ("gamma", "beta,alpha,gamma", "gamma,beta,alpha"),
            ("alpha", "", ""),
        ] {
            let order = CatalogOrder {
                names: names(saved),
            };
            assert_eq!(
                order.applied_to(&names(installed)),
                names(expected),
                "{saved} / {installed}"
            );
        }
    }

    #[test]
    fn reorder_accepts_only_each_installed_app_once() {
        for (installed, requested, expected) in [
            ("alpha,beta", "beta,alpha", Ok("beta,alpha")),
            ("alpha,beta", "alpha,beta", Ok("alpha,beta")),
            ("", "", Ok("")),
            (
                "alpha,beta",
                "",
                Err("Catalog order must list each installed App once"),
            ),
            (
                "alpha,beta",
                "alpha",
                Err("Catalog order must list each installed App once"),
            ),
            (
                "alpha,beta",
                "beta,alpha,gamma",
                Err("Catalog order names an App that is not installed"),
            ),
            (
                "alpha,beta",
                "alpha,missing",
                Err("Catalog order names an App that is not installed"),
            ),
            (
                "",
                "alpha",
                Err("Catalog order names an App that is not installed"),
            ),
            (
                "alpha,beta",
                "alpha,alpha",
                Err("Catalog order lists an App more than once"),
            ),
            (
                "alpha,beta",
                "beta,alpha,beta",
                Err("Catalog order lists an App more than once"),
            ),
        ] {
            let result = CatalogOrder::reorder(&names(installed), &names(requested));
            match expected {
                Ok(order) => assert_eq!(result.unwrap().names, names(order), "{requested}"),
                Err(error) => assert_eq!(result.unwrap_err(), error, "{requested}"),
            }
        }
    }
}
