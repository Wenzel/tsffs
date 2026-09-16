// Copyright (C) 2024 Intel Corporation
// SPDX-License-Identifier: Apache-2.0

//! UEFI module discovery (UCOV-M2, milestone-scope steps 1-2).
//!
//! # Background
//!
//! Unlike Windows (`crate::os::windows`), Simics has no native C interface for
//! UEFI module discovery -- there is no `osa_target_info` (checked directly
//! against the Simics 6/7 headers). The only mechanism confirmed to work is the
//! `uefi_fw_tracker` component's `list-modules` CLI command, invoked from Rust via
//! `simics::api::simulator::script::run_command(String) -> Result<AttrValue>`
//! (e.g. `run_command("$system.soft.tracker.list-modules max = 1000")`, where the
//! `$system.soft.tracker` object path is board-specific and must be supplied by
//! the caller, not hardcoded). Calling `run_command` for real, and everything
//! downstream of it (wiring into `crate::haps`/`HARNESS_START`, a `self.uefi`
//! attribute on `Tsffs`, touching the OS enum), is explicitly out of scope for
//! this milestone -- see the UCOV-M2 spec's milestone-scope step 3.
//!
//! This module implements only the two pieces of that spec that are testable
//! completely offline, with no live Simics session and no real BIOS/UEFI image:
//!
//! 1. [`parse_module_list`]: parse the `AttrValue`/`AttrValueType` shape
//!    `list-modules` returns into `(name, base, size, embedded_path)` tuples.
//! 2. [`UefiOsInfo::resolve`]: given those tuples and a local build-root
//!    directory, resolve each module's real local debug-info path.
//!
//! # Why `AttrValueType`, not `AttrValue`, as the parser's input type
//!
//! `simics::AttrValue` is a `#[repr(C)]` wrapper around the C `attr_value_t`
//! union (see `simics::api::base::attr_value`). Reading a real, already-populated
//! `AttrValue` (e.g. one actually returned by `run_command`) is safe pure memory
//! access with no FFI call (`AttrValue::as_heterogeneous_list`/`as_heterogeneous_dict`
//! just walk `private_u.list`/`private_u.dict` pointers). But *constructing* an
//! owned `AttrValue::List`/`AttrValue::Dict` from scratch (e.g. `AttrValue::list(n)`,
//! or the `TryFrom<Vec<T>>`/`TryFrom<BTreeMap<T, U>>` impls that a hand-built fake
//! fixture would need) allocates through `SIM_alloc_attr_list`/`SIM_alloc_attr_dict`
//! -- real FFI entry points into `libsimics-common.dll`. Exactly like the
//! `get_object("tsffs")` call removed from `SourceCache::new` (see
//! `src/source_cov/mod.rs` and `tests/dwarf_fixture.rs`'s module doc), calling any
//! `SIM_*` entry point with no live Simics session hard-aborts the process, not
//! just returns `Err`. `AttrValueType` (the plain Rust tagged-union enum
//! `Invalid | Nil | Unsigned(u64) | Signed(i64) | Bool(bool) | String(String) |
//! Float(..) | Object(*mut ConfObject) | Data(Box<[u8]>) | List(Vec<Self>) |
//! Dict(BTreeMap<Self, Self>)`) has no such constructors -- its variants are built
//! with plain Rust syntax, no FFI at all -- so it is what this module's parser
//! takes, and what the offline tests construct fixtures as. At a real call site,
//! converting the real `AttrValue` returned by `run_command` into `AttrValueType`
//! via `.into()` (`impl From<AttrValue> for AttrValueType`) is the safe, pure-read
//! conversion described above; this module never needs to go the other direction.
//!
//! # Assumed shape of `list-modules`' return value
//!
//! There is no live Simics session available to inspect the real
//! `uefi_fw_tracker.list-modules` return value in this environment, so this shape
//! is an explicit, documented assumption (per the spec's own instruction to "pick
//! a reasonable representation" and document it), not a confirmed fact:
//!
//! - The top-level value is a `List` of rows.
//! - Each row is a `Dict` keyed by column name (rather than a positional `List`,
//!   i.e. a tuple/row-of-columns) with `String` keys:
//!   - `"name"` -> `String`: the module's **full embedded build-machine path**
//!     (e.g. `/home/robertgu/mydev/.../DEBUG/PeiCore.efi`), confirmed by a prior
//!     investigation (2026-03-12 live tracker dump) to be the full untruncated
//!     path, not the truncated basename shown in the interactive CLI table.
//!   - `"base"` -> `Unsigned` (or `Signed`, if non-negative): the module's
//!     loaded/base address.
//!   - `"size"` -> `Unsigned` (or `Signed`, if non-negative): the module's size in
//!     bytes.
//!
//!   A dict keyed by column name was picked over a positional list-of-columns
//!   representation because it's self-describing and robust to `list-modules`
//!   reordering or adding columns, and because Simics CLI commands that return
//!   per-row structured data commonly do so as attribute dicts. If a real
//!   `list-modules` return value turns out to instead be a positional list, only
//!   [`parse_module_row`] needs to change; [`parse_module_list`]'s and
//!   [`UefiOsInfo::resolve`]'s contracts are unaffected.
//! - This module's own output "name" (in the `(name, base, size, embedded_path)`
//!   tuple) is *not* read from a raw field -- it's derived from `"name"`'s full
//!   path via [`Path::file_name`], e.g. `PeiCore.efi`.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, bail, Result};
use simics::AttrValueType;
use tracing::{debug, warn};
use walkdir::WalkDir;

use crate::util::path_suffix_index::PathSuffixIndex;

/// Parse the `AttrValueType` shape `list-modules` returns (see the module doc
/// comment for the assumed shape) into `(name, base, size, embedded_path)`
/// tuples, where `name` is the bare filename extracted from `embedded_path`.
pub fn parse_module_list(value: &AttrValueType) -> Result<Vec<(String, u64, u64, PathBuf)>> {
    let AttrValueType::List(rows) = value else {
        bail!(
            "expected list-modules result to be an AttrValueType::List, got {:?}",
            value
        );
    };

    rows.iter().map(parse_module_row).collect()
}

/// Parse a single row of the assumed `list-modules` shape.
fn parse_module_row(row: &AttrValueType) -> Result<(String, u64, u64, PathBuf)> {
    let AttrValueType::Dict(fields) = row else {
        bail!(
            "expected each list-modules row to be an AttrValueType::Dict, got {:?}",
            row
        );
    };

    let embedded_path = PathBuf::from(dict_get_string(fields, "name")?);
    let base = dict_get_unsigned(fields, "base")?;
    let size = dict_get_unsigned(fields, "size")?;

    let name = embedded_path
        .file_name()
        .and_then(|n| n.to_str())
        .map(str::to_string)
        .ok_or_else(|| {
            anyhow!(
                "embedded path {:?} in list-modules row has no file name component",
                embedded_path
            )
        })?;

    Ok((name, base, size, embedded_path))
}

fn dict_get<'a>(
    fields: &'a BTreeMap<AttrValueType, AttrValueType>,
    key: &str,
) -> Result<&'a AttrValueType> {
    fields
        .get(&AttrValueType::String(key.to_string()))
        .ok_or_else(|| anyhow!("list-modules row missing expected field {:?}", key))
}

fn dict_get_string(fields: &BTreeMap<AttrValueType, AttrValueType>, key: &str) -> Result<String> {
    match dict_get(fields, key)? {
        AttrValueType::String(s) => Ok(s.clone()),
        other => bail!("expected field {:?} to be a String, got {:?}", key, other),
    }
}

fn dict_get_unsigned(fields: &BTreeMap<AttrValueType, AttrValueType>, key: &str) -> Result<u64> {
    match dict_get(fields, key)? {
        AttrValueType::Unsigned(u) => Ok(*u),
        AttrValueType::Signed(s) if *s >= 0 => Ok(*s as u64),
        other => bail!(
            "expected field {:?} to be an unsigned integer, got {:?}",
            key,
            other
        ),
    }
}

/// UEFI/SMM module debug-info info, resolved from a `list-modules` dump plus a
/// local build-root directory.
///
/// Unlike `crate::os::windows::WindowsOsInfo`, which keys most of its state by
/// CPU index (`HashMap<i32, ...>`) because Windows tracks per-CPU current
/// process/module state, UEFI/SMM has no such per-CPU context -- it's a single
/// flat address space/module list -- so this holds a flat `Vec` instead.
#[derive(Debug, Clone, Default)]
pub struct UefiOsInfo {
    /// Resolved modules: `(name, base, resolved_local_debug_path)`. Feeding this
    /// into the DWARF milestone's `DwarfModule::new(name, base, object)` (which
    /// needs the `object::File` parsed from the path at `resolved_local_debug_path`)
    /// is explicitly out of scope for this milestone.
    pub modules: Vec<(String, u64, PathBuf)>,
}

impl UefiOsInfo {
    /// Resolve local debug-info paths for a parsed module list against a local
    /// build-root directory.
    ///
    /// For each module:
    /// 1. Try matching the module's embedded path against a
    ///    [`PathSuffixIndex`] built over `build_root`, longest suffix first. This
    ///    disambiguates same-named modules whose embedded paths differ in a
    ///    parent directory that also exists locally (e.g. two different EDK2
    ///    package subdirectories).
    /// 2. If that finds nothing, fall back to a bare-filename-stem search
    ///    (`rglob`-equivalent walk) under `build_root`.
    /// 3. If, after both, more than one candidate remains ambiguous, log a
    ///    warning and take the first (sorted, for determinism) candidate --
    ///    "fail open", the spec's own explicit decision, rather than erroring out
    ///    or dropping the module.
    pub fn resolve<P>(modules: &[(String, u64, u64, PathBuf)], build_root: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let build_root = build_root.as_ref();
        // `PathSuffixIndex::build_from_dir` does not hash file contents (unlike
        // `SourceCache::new`), which is the whole point of factoring it out of
        // `SourceCache` -- see `src/util/path_suffix_index.rs`'s module doc.
        let index = PathSuffixIndex::build_from_dir(build_root)?;

        let mut resolved = Vec::with_capacity(modules.len());

        for (name, base, _size, embedded_path) in modules {
            let local_path = resolve_one(&index, build_root, name, embedded_path)?;
            resolved.push((name.clone(), *base, local_path));
        }

        Ok(Self { modules: resolved })
    }
}

/// Resolve a single module's local debug-info path. See
/// [`UefiOsInfo::resolve`]'s doc comment for the algorithm.
fn resolve_one(
    index: &PathSuffixIndex,
    build_root: &Path,
    name: &str,
    embedded_path: &Path,
) -> Result<PathBuf> {
    if let Some(local_path) = index.lookup_str_unambiguous(&embedded_path.to_string_lossy()) {
        debug!(
            "resolved module {name:?} via path-suffix match: {embedded_path:?} -> {local_path:?}"
        );
        return Ok(local_path.to_path_buf());
    }

    // Fall back to a bare-filename-stem search, since the suffix index found no
    // match at all (e.g. the embedded path's parent directories don't exist
    // locally under any name that matches).
    let stem = embedded_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(name);

    let mut candidates = find_by_stem(build_root, stem)?;
    // Sort for deterministic "take the first" behavior below.
    candidates.sort();

    match candidates.len() {
        0 => bail!(
            "no local debug info found for module {:?} (embedded path {:?}, build root {:?})",
            name,
            embedded_path,
            build_root
        ),
        1 => {
            debug!(
                "resolved module {name:?} via bare-stem fallback: {embedded_path:?} -> {:?}",
                candidates[0]
            );
            Ok(candidates.remove(0))
        }
        n => {
            // Fail open: log and take the first (sorted) match rather than
            // erroring out or dropping the module -- this is the spec's own
            // explicit decision, matching the `warn!`/`debug!` logging style
            // already used for similar disambiguation situations in
            // `crate::os::windows` (see e.g. `src/os/windows/structs.rs`'s
            // module-lookup logging). Unlike those call sites, this uses the
            // plain `tracing` crate rather than `simics::warn!`/`simics::debug!`:
            // the latter require a live `ConfObject` (e.g.
            // `get_object("tsffs")?`) and call real `SIM_*` FFI entry points,
            // which -- exactly like the bug fixed in `SourceCache::new` -- hard-
            // abort the process with no live Simics session, which is
            // unconditionally true for this milestone's offline scope.
            warn!(
                "ambiguous local debug info for module {name:?}: {n} candidates matched stem \
                 {stem:?} with no unique path-suffix match (embedded path {embedded_path:?}); \
                 taking the first candidate (fail open): {:?}",
                candidates[0]
            );
            Ok(candidates.remove(0))
        }
    }
}

/// Find every local file under `root` whose file stem (file name without its
/// final extension) matches `stem`.
fn find_by_stem(root: &Path, stem: &str) -> Result<Vec<PathBuf>> {
    Ok(WalkDir::new(root)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.path().file_stem().and_then(|s| s.to_str()) == Some(stem))
        .map(|entry| entry.path().to_path_buf())
        .collect())
}
