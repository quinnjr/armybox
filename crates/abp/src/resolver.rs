//! Dependency Resolution
//!
//! Resolves package dependencies and determines installation order.

extern crate alloc;

use crate::database::Database;
use crate::format::Dependency;
use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;

/// Resolve dependencies for a set of packages
///
/// Returns the packages in installation order (dependencies first).
pub fn resolve_dependencies(db: &Database, packages: &[String]) -> Result<Vec<String>, String> {
    let local: Vec<(String, Vec<String>)> = Vec::new();
    resolve_with_local(db, packages, &local)
}

/// Resolve dependencies with knowledge of local packages being installed.
///
/// `local_packages` is a list of (name, depends) for packages being installed
/// from local .abp files that aren't yet in the database or available index.
pub fn resolve_with_local(
    db: &Database,
    packages: &[String],
    local_packages: &[(String, Vec<String>)],
) -> Result<Vec<String>, String> {
    let mut resolved: Vec<String> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut stack: BTreeSet<String> = BTreeSet::new();

    // Build a lookup for local packages
    let mut local_map: alloc::collections::BTreeMap<String, Vec<String>> =
        alloc::collections::BTreeMap::new();
    for (name, deps) in local_packages {
        local_map.insert(name.clone(), deps.clone());
    }

    for pkg in packages {
        resolve_package_local(db, pkg, &local_map, &mut resolved, &mut seen, &mut stack)?;
    }

    Ok(resolved)
}

fn resolve_package_local(
    db: &Database,
    name: &str,
    local_map: &alloc::collections::BTreeMap<String, Vec<String>>,
    resolved: &mut Vec<String>,
    seen: &mut BTreeSet<String>,
    stack: &mut BTreeSet<String>,
) -> Result<(), String> {
    if seen.contains(name) {
        return Ok(());
    }

    if stack.contains(name) {
        return Err(format_err("circular dependency detected: ", name));
    }

    stack.insert(String::from(name));

    // Get dependencies (check local packages first, then DB)
    let deps = if let Some(deps) = local_map.get(name) {
        deps.clone()
    } else {
        get_dependencies(db, name)?
    };

    for dep in deps {
        let dep_parsed = Dependency::parse(&dep);

        // Already installed?
        if let Some(installed) = db.get_package(&dep_parsed.name)
            && dep_parsed.satisfied_by(&installed.version)
        {
            continue;
        }

        // Being installed locally?
        if local_map.contains_key(&dep_parsed.name) {
            resolve_package_local(db, &dep_parsed.name, local_map, resolved, seen, stack)?;
            continue;
        }

        // Available in repo?
        if let Some(available) = db.get_available(&dep_parsed.name)
            && dep_parsed.satisfied_by(&available.version)
        {
            resolve_package_local(db, &dep_parsed.name, local_map, resolved, seen, stack)?;
            continue;
        }

        // Provided by another package?
        if let Some(provider) = find_provider(db, &dep_parsed.name) {
            resolve_package_local(db, &provider, local_map, resolved, seen, stack)?;
            continue;
        }

        return Err(format_err("unresolved dependency: ", &dep));
    }

    stack.remove(name);
    seen.insert(String::from(name));
    resolved.push(String::from(name));

    Ok(())
}

fn get_dependencies(db: &Database, name: &str) -> Result<Vec<String>, String> {
    // Check if it's an available package
    if let Some(pkg) = db.get_available(name) {
        return Ok(pkg.depends);
    }

    // Check if it's a local package being installed
    // (this would need to be passed in separately in a real implementation)

    // Package not found
    if !db.is_installed(name) {
        return Err(format_err("package not found: ", name));
    }

    // Already installed, get deps from database
    if let Some(pkg) = db.get_package(name) {
        return Ok(pkg.depends);
    }

    Ok(Vec::new())
}

fn find_provider(db: &Database, name: &str) -> Option<String> {
    // Check installed packages
    for pkg in db.list_packages() {
        if pkg
            .provides
            .iter()
            .any(|p| p == name || p.starts_with(&format_provider(name)))
        {
            return Some(pkg.name);
        }
    }

    None
}

fn format_provider(name: &str) -> String {
    let mut s = String::from(name);
    s.push('=');
    s
}

fn format_err(prefix: &str, suffix: &str) -> String {
    let mut s = String::from(prefix);
    s.push_str(suffix);
    s
}

/// Check if a set of packages can be installed together
pub fn check_conflicts(_db: &Database, _packages: &[String]) -> Result<(), String> {
    // TODO: Implement conflict checking
    // This would check:
    // 1. File conflicts between packages
    // 2. Explicit conflicts in package metadata
    // 3. Incompatible version requirements

    Ok(())
}

/// Determine packages that would be removed by an upgrade
pub fn find_obsolete(db: &Database, new_packages: &[String]) -> Vec<String> {
    let obsolete = Vec::new();

    for pkg_name in new_packages {
        if let Some(_available) = db.get_available(pkg_name) {
            // Check replaces field
            // (not implemented in our simple metadata format)
        }
    }

    obsolete
}

/// Topological sort of packages based on dependencies
pub fn topological_sort(packages: &[(String, Vec<String>)]) -> Result<Vec<String>, String> {
    let mut result = Vec::new();
    let mut visited: BTreeSet<String> = BTreeSet::new();
    let mut temp_visited: BTreeSet<String> = BTreeSet::new();

    // Build name -> deps map
    let mut dep_map: alloc::collections::BTreeMap<String, Vec<String>> =
        alloc::collections::BTreeMap::new();
    for (name, deps) in packages {
        dep_map.insert(name.clone(), deps.clone());
    }

    fn visit(
        name: &str,
        dep_map: &alloc::collections::BTreeMap<String, Vec<String>>,
        visited: &mut BTreeSet<String>,
        temp_visited: &mut BTreeSet<String>,
        result: &mut Vec<String>,
    ) -> Result<(), String> {
        if visited.contains(name) {
            return Ok(());
        }
        if temp_visited.contains(name) {
            return Err(format_err("cycle detected at: ", name));
        }

        temp_visited.insert(String::from(name));

        if let Some(deps) = dep_map.get(name) {
            for dep in deps {
                // Only visit if it's in our package set
                if dep_map.contains_key(dep) {
                    visit(dep, dep_map, visited, temp_visited, result)?;
                }
            }
        }

        temp_visited.remove(name);
        visited.insert(String::from(name));
        result.push(String::from(name));

        Ok(())
    }

    for (name, _) in packages {
        visit(name, &dep_map, &mut visited, &mut temp_visited, &mut result)?;
    }

    Ok(result)
}
