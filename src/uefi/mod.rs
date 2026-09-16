// Copyright (C) 2024 Intel Corporation
// SPDX-License-Identifier: Apache-2.0

//! UEFI module discovery (UCOV-M2, milestone-scope steps 1-2).
//!
//! # Background
//!
//! Unlike Windows (`crate::os::windows`), Simics has no native C interface for
//! UEFI module discovery -- there is no `osa_target_info` (checked directly
//! against the Simics 6/7 headers). The confirmed-real mechanism, found via a
//! live investigation (real Simics 6.0.189 session, a real
//! checkpoint past DXE dispatch, 68 real loaded UEFI modules), is the
//! `uefi_fw_tracker` component's underlying C object's `maps` attribute, reached
//! from Rust via Simics's CLI arrow-attribute syntax and the exact same FFI
//! entry point TSFFS already uses elsewhere in this module's design:
//! `simics::api::simulator::script::run_command(String) -> Result<AttrValue>`
//! (e.g. `run_command("tracker_obj->maps")`, where the
//! `qsp.software.tracker` object path is board-specific and must be supplied by
//! the caller, not hardcoded). This supersedes an earlier design that queried the
//! tracker's `list-modules` CLI command instead: `list-modules` itself calls
//! `basename()` on the underlying data before returning it, so it only ever
//! yields a bare filename, never a full path -- `tracker_obj->maps` is the same
//! underlying data with the full path intact. Calling `run_command` for real,
//! and everything downstream of it (wiring into `crate::haps`/`HARNESS_START`, a
//! `self.uefi` attribute on `Tsffs`, touching the OS enum), is explicitly out of
//! scope for this milestone -- see the UCOV-M2 spec's milestone-scope step 3.
//!
//! This module implements only the two pieces of that spec that are testable
//! completely offline, with no live Simics session and no real BIOS/UEFI image:
//!
//! 1. [`parse_module_list`]: parse the `AttrValue`/`AttrValueType` shape
//!    `tracker_obj->maps` returns into `(name, base, size, embedded_path)`
//!    tuples.
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
//! # Confirmed shape of `tracker_obj->maps`' return value
//!
//! Unlike the superseded `list-modules`-based design (which had to *assume* a
//! shape, since it was never actually queried live), this shape is a confirmed
//! fact, captured from a real live tracker dump reached from Rust via
//! `run_command`, using the exact same FFI path this module documents above:
//!
//! - The top-level value is a `List` of rows.
//! - Each row is itself a positional `List` of exactly 7 elements (**not** a dict
//!   keyed by column name, unlike the superseded design):
//!
//!   ```text
//!   [loaded_address, loaded_size, <bool>, adjusted_address, adjusted_size, <bool>, full_path_string]
//!   ```
//!
//!   A real captured example row:
//!
//!   ```text
//!   [3744034816, 189184, True, 3744034816, 189184, True,
//!    '/home/user/bios-x58i/project/workspace/Build/SimicsOpenBoardPkg/BoardX58Ich10/DEBUG_GCC/X64/MdeModulePkg/Core/Dxe/DxeMain/DEBUG/DxeCore.efi']
//!   ```
//!
//!   This module reads only 3 of the 7 elements:
//!   - index 0 (`loaded_address`) -> `Unsigned`/`Signed`: the module's loaded/base
//!     address. (`index 3`, `adjusted_address`, is a distinct post-relocation
//!     address also present in the real data, but out of scope for this
//!     milestone -- nothing downstream of this module currently consumes it.)
//!   - index 1 (`loaded_size`) -> `Unsigned`/`Signed`: the module's size in bytes.
//!   - index 6 (`full_path_string`) -> `String`, **or absent**: the module's
//!     full embedded build-machine path. Some rows genuinely have no path at
//!     all -- real "unknown"/unresolved modules that the tracker could not
//!     identify (mirroring `module_load.py`'s own upstream guard for
//!     `m['image'] is None`). Confirmed live against the real,
//!     full 68-row `tracker_obj->maps` capture (not just the samples from the
//!     initial investigation): exactly one real row (of 68) is genuinely
//!     pathless, and its Python value is `None`, not an empty string -- which
//!     `simics::AttrValueType::from(AttrValue)` (`is_nil()` checked first)
//!     converts to `AttrValueType::Nil`, **not** `AttrValueType::String(String::new())`.
//!     An earlier revision of this module assumed the latter (an empty
//!     string) and hard-errored on the real `Nil` case; [`list_get_string_or_nil`]
//!     now accepts both `Nil` and (defensively) an empty `String` as "no
//!     path". Unlike the superseded design, there is no separate "name" field
//!     at all: with this row shape, a module's short display name must be
//!     *derived* from the full path via [`Path::file_name`] when a path is
//!     present, e.g. `DxeCore.efi` from the example above. See
//!     [`parse_module_row`] for how a missing path is named instead
//!     (`<unknown>`).
//!   - indices 2, 4, 5 (the two booleans and `adjusted_size`) are not read by
//!     this module; they are out of scope for this milestone.
//!
//! A real, observed duplicate-name case -- two loaded instances of
//! `BootScriptExecutorDxe.efi`, at two different addresses, seen both via
//! `list-modules` and via `tracker_obj->maps` -- was re-examined under this
//! richer source and turned out to have the **identical** full path for both
//! instances (the same build loaded twice, not two different binaries). So for
//! that specific real case, path-suffix disambiguation is unnecessary -- any
//! single matching local file is correct for both addresses. This does *not*
//! prove disambiguation is unnecessary in general: two genuinely different
//! builds sharing a basename (e.g. two different EDK2 package subdirectories)
//! remains a real possibility this module still needs to handle correctly, since
//! that scenario is not disproven for all cases, just this one -- see
//! [`UefiOsInfo::resolve`]'s doc comment.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Result};
use simics::AttrValueType;
use tracing::{debug, warn};
use walkdir::WalkDir;

use crate::util::path_suffix_index::PathSuffixIndex;

/// Placeholder name used for a module whose row has no path at all (a real
/// "unknown"/unresolved module -- see the module doc comment's "Confirmed shape"
/// section). Matches the placeholder `list-modules` itself used for such rows in
/// the real capture that motivated this design.
pub const UNKNOWN_MODULE_NAME: &str = "<unknown>";

/// Parse the `AttrValueType` shape `tracker_obj->maps` returns (see the module
/// doc comment for the confirmed shape) into `(name, base, size, embedded_path)`
/// tuples, where `name` is the bare filename extracted from `embedded_path`, or
/// [`UNKNOWN_MODULE_NAME`] when a row has no path.
pub fn parse_module_list(value: &AttrValueType) -> Result<Vec<(String, u64, u64, PathBuf)>> {
    let AttrValueType::List(rows) = value else {
        bail!(
            "expected tracker_obj->maps result to be an AttrValueType::List, got {:?}",
            value
        );
    };

    rows.iter().map(parse_module_row).collect()
}

/// Parse a single row of the confirmed `tracker_obj->maps` shape:
/// `[loaded_address, loaded_size, <bool>, adjusted_address, adjusted_size,
/// <bool>, full_path_string]`. Only indices 0 (`base`), 1 (`size`), and 6
/// (`embedded_path`) are read; see the module doc comment for why the others are
/// out of scope.
fn parse_module_row(row: &AttrValueType) -> Result<(String, u64, u64, PathBuf)> {
    let AttrValueType::List(elements) = row else {
        bail!(
            "expected each tracker_obj->maps row to be an AttrValueType::List, got {:?}",
            row
        );
    };

    let Ok([loaded_address, loaded_size, _, _adjusted_address, _adjusted_size, _, full_path]) =
        <[AttrValueType; 7]>::try_from(elements.clone())
    else {
        bail!(
            "expected each tracker_obj->maps row to have exactly 7 elements, got {}: {:?}",
            elements.len(),
            row
        );
    };

    let base = list_get_unsigned(&loaded_address, 0)?;
    let size = list_get_unsigned(&loaded_size, 1)?;
    let embedded_path_str = list_get_string_or_nil(&full_path, 6)?;

    // A row with a genuinely unresolved module has no path at all -- confirmed
    // live against the real, full 68-row capture to be represented as
    // `AttrValueType::Nil` (Python `None`), not an empty string -- see the
    // module doc comment's "Confirmed shape" section. `list_get_string_or_nil`
    // also defensively accepts an empty string as "no path", in case some
    // other tracker/board configuration ever produces one instead of `Nil`.
    // Treat either as "no path" and fall back to a fixed placeholder name
    // rather than deriving an empty/panic-inducing name from it.
    let (name, embedded_path) = match embedded_path_str.filter(|s| !s.is_empty()) {
        None => (UNKNOWN_MODULE_NAME.to_string(), PathBuf::new()),
        Some(embedded_path_str) => {
            let embedded_path = PathBuf::from(&embedded_path_str);
            let name = embedded_path
                .file_name()
                .and_then(|n| n.to_str())
                .map(str::to_string)
                .ok_or_else(|| {
                    anyhow!(
                        "embedded path {:?} in tracker_obj->maps row has no file name component",
                        embedded_path
                    )
                })?;
            (name, embedded_path)
        }
    };

    Ok((name, base, size, embedded_path))
}

/// Read a positional `tracker_obj->maps` row element expected to be an unsigned
/// (or non-negative signed) integer, e.g. `loaded_address`/`loaded_size`.
/// `index` is only used to produce a helpful error message.
fn list_get_unsigned(element: &AttrValueType, index: usize) -> Result<u64> {
    match element {
        AttrValueType::Unsigned(u) => Ok(*u),
        AttrValueType::Signed(s) if *s >= 0 => Ok(*s as u64),
        other => bail!(
            "expected tracker_obj->maps row element {index} to be an unsigned integer, got {:?}",
            other
        ),
    }
}

/// Read a positional `tracker_obj->maps` row element expected to be either a
/// string or absent, i.e. `full_path_string`. Returns `Ok(None)` for a
/// genuinely pathless row -- confirmed live (see the module doc comment) to
/// arrive as `AttrValueType::Nil` (Python `None`), not an empty string, though
/// an empty string is also accepted defensively and treated the same as `Nil`.
/// `index` is only used to produce a helpful error message.
fn list_get_string_or_nil(element: &AttrValueType, index: usize) -> Result<Option<String>> {
    match element {
        AttrValueType::String(s) => Ok(Some(s.clone())),
        AttrValueType::Nil => Ok(None),
        other => bail!(
            "expected tracker_obj->maps row element {index} to be a String or Nil, got {:?}",
            other
        ),
    }
}

/// UEFI/SMM module debug-info info, resolved from a `tracker_obj->maps` dump plus
/// a local build-root directory.
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
    /// Under the superseded `list-modules`-based design, an embedded path was
    /// assumed to be a rare bonus (`list-modules` itself only ever exposed a
    /// bare basename, since it calls `basename()` internally), so path-suffix
    /// matching was a secondary "if we ever get a path" capability and
    /// bare-stem search was the primary path. `tracker_obj->maps` inverts that:
    /// a full embedded path is the *common* case (every genuinely-identified
    /// module has one; see the module doc comment), so path-suffix matching is
    /// now the primary resolution path.
    ///
    /// For each module:
    /// 1. If the module has no embedded path at all (a genuinely pathless row
    ///    -- a real "unknown"/unresolved module, not merely a basename-only
    ///    row), fail that module explicitly rather than guessing -- see
    ///    [`resolve_one`].
    /// 2. Otherwise, try matching the module's embedded path against a
    ///    [`PathSuffixIndex`] built over `build_root`, longest suffix first.
    ///    This disambiguates same-named modules whose embedded paths differ in
    ///    a parent directory that also exists locally (e.g. two different EDK2
    ///    package subdirectories) -- the primary resolution path, and expected
    ///    to resolve the overwhelming majority of real modules outright, since
    ///    they carry a full embedded path.
    /// 3. If that finds nothing (e.g. the embedded path's parent directories
    ///    don't exist locally under any matching name), fall back to a
    ///    bare-filename-stem search (`rglob`-equivalent walk) under
    ///    `build_root`.
    /// 4. If, after both, more than one candidate remains ambiguous, log a
    ///    warning and take the first (sorted, for determinism) candidate --
    ///    "fail open", the spec's own explicit decision, rather than erroring out
    ///    or dropping the module. In practice this fires rarely now: the one
    ///    real observed duplicate-name case investigated (two
    ///    `BootScriptExecutorDxe.efi` instances) turned out to share an
    ///    identical full path (the same build loaded twice), which step 2
    ///    resolves outright with no ambiguity at all.
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
    if embedded_path.as_os_str().is_empty() {
        // A genuinely pathless row -- a real "unknown"/unresolved module (see
        // `parse_module_row`/`UNKNOWN_MODULE_NAME`), not merely a basename-only
        // row (that case can't arise from `tracker_obj->maps`: every row that
        // has a path at all has a *full* path, never just a basename). There is
        // no embedded path to suffix-match against, and no real file name to
        // bare-stem-search by either -- `name` here is just the
        // `UNKNOWN_MODULE_NAME` placeholder, not a real file name, so searching
        // for it would either find nothing or silently match an unrelated local
        // file that happens to share that placeholder name. Fail this module
        // explicitly instead.
        bail!(
            "module {name:?} has no embedded path (unknown/unresolved module); cannot resolve \
             local debug info"
        );
    }

    // Primary resolution path (see `UefiOsInfo::resolve`'s doc comment for why
    // this now comes first): match the module's full embedded path against a
    // `PathSuffixIndex` built over `build_root`, longest suffix first.
    if let Some(local_path) = index.lookup_str_unambiguous(&embedded_path.to_string_lossy()) {
        debug!(
            "resolved module {name:?} via path-suffix match: {embedded_path:?} -> {local_path:?}"
        );
        return Ok(local_path.to_path_buf());
    }

    // Fall back to a bare-filename-stem search, since the suffix index found no
    // match at all (e.g. the embedded path's parent directories don't exist
    // locally under any name that matches). This is now specifically a
    // fallback for that case, not the primary path -- see `UefiOsInfo::resolve`'s
    // doc comment.
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
