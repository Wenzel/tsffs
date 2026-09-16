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
//! the caller, not hardcoded -- confirmed live to be `qsp.software.tracker` on the
//! `examples/tutorials/edk2-simics-platform` tutorial a live checkpoint, see below).
//! Calling `run_command` for real, and everything downstream of it (wiring into
//! `crate::haps`/`HARNESS_START`, a `self.uefi` attribute on `Tsffs`, touching the
//! OS enum), is explicitly out of scope for this milestone -- see the UCOV-M2
//! spec milestone-scope step 3.
//!
//! This module implements only the two pieces of that spec that are testable
//! completely offline, with no live Simics session and no real BIOS/UEFI image:
//!
//! 1. [`parse_module_list`]: parse the `AttrValue`/`AttrValueType` shape
//!    `list-modules` returns into `(name, base, size, embedded_path)` tuples.
//! 2. [`UefiOsInfo::resolve`]: given those tuples and a local build-root
//!    directory, resolve each module real local debug-info path.
//!
//! # Why `AttrValueType`, not `AttrValue`, as the parser input type
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
//! `src/source_cov/mod.rs` and `tests/dwarf_fixture.rs` module doc), calling any
//! `SIM_*` entry point with no live Simics session hard-aborts the process, not
//! just returns `Err`. `AttrValueType` (the plain Rust tagged-union enum
//! `Invalid | Nil | Unsigned(u64) | Signed(i64) | Bool(bool) | String(String) |
//! Float(..) | Object(*mut ConfObject) | Data(Box<[u8]>) | List(Vec<Self>) |
//! Dict(BTreeMap<Self, Self>)`) has no such constructors -- its variants are built
//! with plain Rust syntax, no FFI at all -- so it is what this module parser
//! takes, and what the offline tests construct fixtures as. At a real call site,
//! converting the real `AttrValue` returned by `run_command` into `AttrValueType`
//! via `.into()` (`impl From<AttrValue> for AttrValueType`) is the safe, pure-read
//! conversion described above; this module never needs to go the other direction.
//!
//! # Confirmed shape of `list-modules` return value
//!
//! This shape was originally an explicit, documented *assumption* (there was no
//! live Simics session available to check it against), but it has since been
//! **confirmed against a real, live Simics session**, and turned out to be wrong
//! in every particular. The confirmation:
//!
//! - On 2026-09-16, on the `the dev host` host, a real a live checkpoint was booted to a
//!   checkpoint (`~/tsffs-bmc-bios-poc/bios-x58i/project/checkpoint.ckpt`, itself
//!   produced from the same `BoardX58Ich10`/`qsp-uefi-custom` setup this crate own
//!   `examples/tutorials/edk2-simics-platform` tutorial uses) with the
//!   `uefi_fw_tracker` inserted and re-enabled (`qsp.software.enable-tracker`)
//!   after loading the checkpoint. The real object path is `qsp.software.tracker`
//!   (not the generic `$system.soft.tracker` placeholder above).
//! - `simics.SIM_run_command("qsp.software.tracker.list-modules max = 1000")` --
//!   the exact Python-level equivalent of this crate own
//!   `run_command(String) -> Result<AttrValue>` -- was called directly, and its
//!   real Python `type()`/`repr()` captured (not the pretty-printed CLI table).
//!   It returned a plain Python `list` of 78 real modules, each itself a plain
//!   Python `list` of 5 elements, e.g.
//!   `['DxeCore.efi', 3744034816, 189184, '', '']`.
//! - This was cross-checked against the `uefi_fw_tracker` component own installed
//!   Python source (`simmod/uefi_fw_tracker/module_load.py` `get_mappings`/
//!   `list_modules`/`mappings_table_properties`), identical across every
//!   installed Simics-Base version checked (6.0.189, 7.74.0, 7.100.0, 7.106.0):
//!   `list-modules` is a generic Simics *table* command
//!   (`table.new_table_command`), and its programmatic return value
//!   (`cli.command_return(value=out_data, ...)`) is `out_data`, a plain list of
//!   `[Module, "Loaded Address", "Size", "Adjusted Address", "Adjusted Size"]`
//!   rows built as `[basename(m['image']), m['loaded_address'], m['loaded_size'],
//!   ...]` -- confirming both the shape and the *reason* for it (it is this
//!   Simics version generic table-command return convention, not anything
//!   UEFI-specific).
//!
//! The confirmed real shape, converted from that live Python `repr()` into
//! `AttrValueType` terms:
//!
//! - The top-level value is a `List` of rows.
//! - Each row is itself a positional `List` (**not** a `Dict` keyed by column
//!   name, as originally assumed), with at least 3 elements:
//!   - `[0]` ("Module") -> `String`: the module bare basename only (e.g.
//!     `DxeCore.efi`), or the literal string `"<unknown>"` if the tracker has no
//!     image name for that mapping (both observed live) -- **not** the full
//!     embedded build-machine path originally assumed. `list-modules` never
//!     exposes that path at all; only the tracker own `params` attribute does
//!     (populated from a locally-loaded `.map` file via `detect-parameters`/
//!     `load-parameters`), which is not applicable here since the whole point of
//!     runtime module discovery is to work without already having that file.
//!   - `[1]` ("Loaded Address") -> an integer (`Unsigned` or `Signed`; the real
//!     capture addresses, e.g. `3744034816`, cross the FFI boundary as `Signed`
//!     for the ranges observed).
//!   - `[2]` ("Size") -> an integer, same representation as `[1]`.
//!   - `[3]`/`[4]` ("Adjusted Address"/"Adjusted Size") -> an integer when the
//!     tracker has separately loaded symbol info at a different address,
//!     otherwise the literal empty `String("")` -- true for every module in the
//!     real capture. This module has no use for either column and does not parse
//!     them; [`parse_module_row`] only requires at least 3 columns to be present.
//! - A **real observed duplicate-name case** confirms the consequence of the
//!   above: `BootScriptExecutorDxe.efi` appeared twice in the live capture, at
//!   two different addresses, with **no other distinguishing information**.
//!   Because `list-modules` never supplies a full path, [`parse_module_row`]
//!   `embedded_path` output for every module is just its bare name (`[0]`)
//!   wrapped in a `PathBuf` -- so [`UefiOsInfo::resolve`] path-suffix
//!   disambiguation phase can never do better than its own bare-stem-match
//!   fallback for real `list-modules`-sourced input. For any real duplicate-name
//!   module, that fallback "fail open" behavior (log a warning, take the first
//!   sorted local candidate) is therefore the **expected**, common outcome, not
//!   a rare edge case -- see [`UefiOsInfo::resolve`] doc comment.
//! - This module own output "name" (in the `(name, base, size, embedded_path)`
//!   tuple) is read directly from row `[0]` -- unlike the original assumption,
//!   there is no full path to extract a bare filename from with
//!   [`Path::file_name`]; row `[0]` already *is* the bare filename.

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use simics::AttrValueType;
use tracing::{debug, warn};
use walkdir::WalkDir;

use crate::util::path_suffix_index::PathSuffixIndex;

/// Parse the `AttrValueType` shape `list-modules` returns (see the module doc
/// comment for the confirmed real shape) into `(name, base, size, embedded_path)`
/// tuples, where `name` is the bare filename `list-modules` itself returns (there
/// is no full path to extract it from), and `embedded_path` is that same bare
/// name wrapped in a `PathBuf` (see the module doc comment for why).
pub fn parse_module_list(value: &AttrValueType) -> Result<Vec<(String, u64, u64, PathBuf)>> {
    let AttrValueType::List(rows) = value else {
        bail!(
            "expected list-modules result to be an AttrValueType::List, got {:?}",
            value
        );
    };

    rows.iter().map(parse_module_row).collect()
}

/// Parse a single row of the confirmed real `list-modules` shape: a positional
/// `List` of at least 3 columns, `[Module, Loaded Address, Size, ..]` -- see the
/// module doc comment. Any columns beyond the first 3 (the real shape has 5,
/// "Adjusted Address"/"Adjusted Size") are ignored; this milestone has no use for
/// them, and requiring only "at least 3" rather than exactly 5 keeps this
/// tolerant of Simics versions/trackers that might add or drop trailing columns.
fn parse_module_row(row: &AttrValueType) -> Result<(String, u64, u64, PathBuf)> {
    let AttrValueType::List(columns) = row else {
        bail!(
            "expected each list-modules row to be an AttrValueType::List (positional \
             columns, not a Dict -- see the module doc comment), got {:?}",
            row
        );
    };

    if columns.len() < 3 {
        bail!(
            "expected each list-modules row to have at least 3 columns (Module, Loaded \
             Address, Size), got {} column(s): {:?}",
            columns.len(),
            row
        );
    }

    let name = column_string(&columns[0], "Module")?;
    let base = column_unsigned(&columns[1], "Loaded Address")?;
    let size = column_unsigned(&columns[2], "Size")?;

    // `list-modules` never returns a full embedded build-machine path (see the
    // module doc comment) -- this bare basename, already extracted by the
    // tracker itself, is all there is.
    let embedded_path = PathBuf::from(&name);

    Ok((name, base, size, embedded_path))
}

fn column_string(value: &AttrValueType, column: &str) -> Result<String> {
    match value {
        AttrValueType::String(s) => Ok(s.clone()),
        other => bail!(
            "expected list-modules column {:?} to be a String, got {:?}",
            column,
            other
        ),
    }
}

fn column_unsigned(value: &AttrValueType, column: &str) -> Result<u64> {
    match value {
        AttrValueType::Unsigned(u) => Ok(*u),
        AttrValueType::Signed(s) if *s >= 0 => Ok(*s as u64),
        other => bail!(
            "expected list-modules column {:?} to be an unsigned integer, got {:?}",
            column,
            other
        ),
    }
}

/// UEFI/SMM module debug-info info, resolved from a `list-modules` dump plus a
/// local build-root directory.
///
/// Unlike `crate::os::windows::WindowsOsInfo`, which keys most of its state by
/// CPU index (`HashMap<i32, ...>`) because Windows tracks per-CPU current
/// process/module state, UEFI/SMM has no such per-CPU context -- it is a single
/// flat address space/module list -- so this holds a flat `Vec` instead.
#[derive(Debug, Clone, Default)]
pub struct UefiOsInfo {
    /// Resolved modules: `(name, base, resolved_local_debug_path)`. Feeding this
    /// into the DWARF milestone `DwarfModule::new(name, base, object)` (which
    /// needs the `object::File` parsed from the path at `resolved_local_debug_path`)
    /// is explicitly out of scope for this milestone.
    pub modules: Vec<(String, u64, PathBuf)>,
}

impl UefiOsInfo {
    /// Resolve local debug-info paths for a parsed module list against a local
    /// build-root directory.
    ///
    /// For each module:
    /// 1. Try matching the module embedded path against a
    ///    [`PathSuffixIndex`] built over `build_root`, longest suffix first. This
    ///    disambiguates same-named modules whose embedded paths differ in a
    ///    parent directory that also exists locally (e.g. two different EDK2
    ///    package subdirectories) -- **when the caller actually has such an
    ///    embedded path to give it**. [`parse_module_list`] itself never can
    ///    (see its module doc comment: real `list-modules` output only ever
    ///    supplies a bare basename, confirmed live), so for input sourced from
    ///    it this phase degenerates to exactly the bare-stem fallback below; it
    ///    remains here as a general capability of this function for any other
    ///    caller/future data source that might supply a real embedded path.
    /// 2. If that finds nothing, fall back to a bare-filename-stem search
    ///    (`rglob`-equivalent walk) under `build_root`.
    /// 3. If, after both, more than one candidate remains ambiguous, log a
    ///    warning and take the first (sorted, for determinism) candidate --
    ///    "fail open", the spec own explicit decision, rather than erroring out
    ///    or dropping the module. For any real duplicate-name module sourced from
    ///    live `list-modules` output, this is the **expected**, common outcome
    ///    (confirmed live: e.g. `BootScriptExecutorDxe.efi` appeared twice with
    ///    no distinguishing information beyond base address), not a rare edge
    ///    case.
    pub fn resolve<P>(modules: &[(String, u64, u64, PathBuf)], build_root: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let build_root = build_root.as_ref();
        // `PathSuffixIndex::build_from_dir` does not hash file contents (unlike
        // `SourceCache::new`), which is the whole point of factoring it out of
        // `SourceCache` -- see `src/util/path_suffix_index.rs` module doc.
        let index = PathSuffixIndex::build_from_dir(build_root)?;

        let mut resolved = Vec::with_capacity(modules.len());

        for (name, base, _size, embedded_path) in modules {
            let local_path = resolve_one(&index, build_root, name, embedded_path)?;
            resolved.push((name.clone(), *base, local_path));
        }

        Ok(Self { modules: resolved })
    }
}

/// Resolve a single module local debug-info path. See
/// [`UefiOsInfo::resolve`] doc comment for the algorithm.
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
    // match at all (e.g. the embedded path parent directories do not exist
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
            // erroring out or dropping the module -- this is the spec own
            // explicit decision, matching the `warn!`/`debug!` logging style
            // already used for similar disambiguation situations in
            // `crate::os::windows` (see e.g. `src/os/windows/structs.rs`
            // module-lookup logging). Unlike those call sites, this uses the
            // plain `tracing` crate rather than `simics::warn!`/`simics::debug!`:
            // the latter require a live `ConfObject` (e.g.
            // `get_object("tsffs")?`) and call real `SIM_*` FFI entry points,
            // which -- exactly like the bug fixed in `SourceCache::new` -- hard-
            // abort the process with no live Simics session, which is
            // unconditionally true for this milestone offline scope.
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
