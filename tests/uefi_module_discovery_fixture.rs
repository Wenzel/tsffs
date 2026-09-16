// Copyright (C) 2024 Intel Corporation
// SPDX-License-Identifier: Apache-2.0

//! End-to-end, offline test of UEFI module discovery milestone-scope steps 1-2
//! (`tsffs::uefi::{parse_module_list, UefiOsInfo}`, UCOV-M2) against fixtures
//! built from a **confirmed real** `list-modules` shape -- see `src/uefi/mod.rs`
//! module doc comment for exactly how and when that shape was confirmed (a live
//! QSP/X58 Simics session on the `vmsifter` host, 2026-09-16) -- mirroring
//! `tests/dwarf_fixture.rs` pattern on the sibling DWARF milestone branch
//! (`feat/dwarf-source-coverage-ucov-m1`).
//!
//! This lives under `tests/` (an integration test / separate cargo crate) rather
//! than as a `#[cfg(test)]` module inside `src/uefi/mod.rs` for the same reason as
//! that file: this crate `[lib]` section sets `test = false`, which disables the
//! implicit unit-test harness for the library target, so `#[cfg(test)]` code
//! inside `src/` is never compiled by `cargo test` for this crate. Integration
//! tests under `tests/` are a separate cargo target and unaffected by that
//! setting. Being a separate crate also means this file only sees `tsffs` `pub`
//! API -- `src/lib.rs` widens `uefi` to `pub` (from `pub(crate)`) for exactly this
//! reason, same as `dwarf`/`source_cov` on the DWARF branch.
//!
//! # A real `list-modules` output was used to build these fixtures
//!
//! Unlike the previous (offline-only, assumption-based) version of this file,
//! the fixtures below are built directly from a real, live tracker capture
//! (`qsp.software.tracker.list-modules max = 1000` against a real QSP/X58
//! checkpoint, on `vmsifter`, 2026-09-16), not a hand-guessed shape -- see
//! `src/uefi/mod.rs` module doc comment ("Confirmed shape of `list-modules`
//! return value") for the full capture and cross-check against the
//! `uefi_fw_tracker` component own installed Python source.
//!
//! # Fixtures
//!
//! [`FIXTURE_ROWS`] models a subset of the real 78-module live capture, with the
//! real observed duplicate-name case (`BootScriptExecutorDxe.efi`, appearing
//! twice at different addresses with no other distinguishing information) and the
//! real observed "no image name known" case (`"<unknown>"`) both included. It is
//! used by both [`parses_confirmed_real_module_list_shape_with_duplicate_names`]
//! (testing [`parse_module_list`] alone) and
//! [`resolve_falls_open_on_the_real_duplicate_name_case`] (testing the full
//! `parse_module_list` -> [`UefiOsInfo::resolve`] pipeline end-to-end).
//!
//! [`resolves_duplicate_names_via_path_suffix_disambiguation_given_full_paths`]
//! separately tests [`UefiOsInfo::resolve`] own generic path-suffix
//! disambiguation capability against hand-built `(name, base, size,
//! embedded_path)` tuples carrying full, distinguishing paths -- real
//! `list-modules` output never supplies such a path (confirmed live, see above),
//! but `UefiOsInfo::resolve` is a generic utility not solely fed from
//! `parse_module_list`, so this capability is still worth testing directly.

use std::{
    collections::HashMap,
    fs::{create_dir_all, write},
    io,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use simics::AttrValueType;
use tempfile::tempdir;
use tracing_subscriber::fmt::MakeWriter;
use tsffs::uefi::{parse_module_list, UefiOsInfo};

/// One row of the confirmed-real-shape fixture: a bare basename (exactly what
/// `list-modules` itself returns, per the module doc comment), a loaded address,
/// and a size. Real rows also carry "Adjusted Address"/"Adjusted Size" columns,
/// empty in the live capture and unused by this milestone; [`fixture_attr_value`]
/// still includes them (as empty strings) for fidelity to the real capture, and
/// `parse_module_row` tolerates that (it only requires at least 3 columns).
struct FixtureRow {
    name: &'static str,
    base: u64,
    size: u64,
}

/// A representative subset of the real 78-row live capture (see the module doc
/// comment in `src/uefi/mod.rs`), including both real observed edge cases: the
/// duplicate-name module (`BootScriptExecutorDxe.efi`, two entries, two
/// addresses, otherwise indistinguishable) and the "no image name known" module
/// (`"<unknown>"`).
const FIXTURE_ROWS: &[FixtureRow] = &[
    FixtureRow {
        name: "DxeCore.efi",
        base: 3_744_034_816,
        size: 189_184,
    },
    FixtureRow {
        name: "PcdDxe.efi",
        base: 3_740_880_896,
        size: 23_680,
    },
    FixtureRow {
        name: "BootScriptExecutorDxe.efi",
        base: 3_739_889_664,
        size: 84_224,
    },
    FixtureRow {
        name: "BootScriptExecutorDxe.efi",
        base: 3_722_764_288,
        size: 84_224,
    },
    FixtureRow {
        name: "<unknown>",
        base: 3_722_997_760,
        size: 195_360,
    },
];

/// Build the real, confirmed `AttrValueType` shape `list-modules` returns (see
/// `src/uefi/mod.rs` module doc comment): a `List` of `List` rows (positional
/// columns, not a `Dict`), each `[Module, "Loaded Address", "Size", "Adjusted
/// Address", "Adjusted Size"]`, with the trailing two columns empty strings --
/// exactly as observed in the real live capture for every module.
fn fixture_attr_value(rows: &[FixtureRow]) -> AttrValueType {
    AttrValueType::List(
        rows.iter()
            .map(|row| {
                AttrValueType::List(vec![
                    AttrValueType::String(row.name.to_string()),
                    AttrValueType::Signed(row.base as i64),
                    AttrValueType::Signed(row.size as i64),
                    AttrValueType::String(String::new()),
                    AttrValueType::String(String::new()),
                ])
            })
            .collect(),
    )
}

#[test]
fn parses_confirmed_real_module_list_shape_with_duplicate_names() -> Result<()> {
    let parsed = parse_module_list(&fixture_attr_value(FIXTURE_ROWS))?;
    assert_eq!(parsed.len(), FIXTURE_ROWS.len());

    for (row, (name, base, size, embedded_path)) in FIXTURE_ROWS.iter().zip(parsed.iter()) {
        assert_eq!(name, row.name);
        assert_eq!(*base, row.base);
        assert_eq!(*size, row.size);
        // Confirmed live: list-modules never supplies a full path, so
        // parse_module_row embedded_path for every module is just the bare
        // name it was given, wrapped.
        assert_eq!(embedded_path, &PathBuf::from(row.name));
    }

    // The real observed duplicate-name case: exactly 2 entries, distinguishable
    // only by base address (not accidentally collapsed/deduplicated by the
    // parser).
    let dup: Vec<_> = parsed
        .iter()
        .filter(|(name, ..)| name == "BootScriptExecutorDxe.efi")
        .collect();
    assert_eq!(
        dup.len(),
        2,
        "expected exactly 2 parsed entries for the real observed duplicate-name module"
    );
    assert_ne!(
        dup[0].1, dup[1].1,
        "the two entries must have distinct base addresses (the only thing distinguishing them)"
    );
    assert_eq!(
        dup[0].3, dup[1].3,
        "list-modules gives both the exact same bare-name embedded_path -- there is no way to \
         tell them apart by path"
    );

    // The real observed "no image name known" case.
    assert!(
        parsed.iter().any(|(name, ..)| name == "<unknown>"),
        "expected the real observed \"<unknown>\" module name to survive parsing unchanged"
    );

    Ok(())
}

/// One row of the hand-built, full-path fixture used only by
/// [`resolves_duplicate_names_via_path_suffix_disambiguation_given_full_paths`]
/// below, to exercise [`UefiOsInfo::resolve`] own generic path-suffix
/// disambiguation capability -- independent of [`parse_module_list`], which
/// (confirmed live) never actually has a full path to supply.
struct HandBuiltModuleRow {
    embedded_path: String,
    base: u64,
    size: u64,
}

/// The fixture common embedded build-machine path prefix, matching the real
/// structure observed in the live capture that this branch investigation
/// confirmed (see `src/uefi/mod.rs` module doc comment).
const PREFIX: &str =
    "/home/mtarral/tsffs-bmc-bios-poc/bios-x58i/project/workspace/Build/SimicsOpenBoardPkg/BoardX58Ich10/DEBUG_GCC/X64";

/// Build 4 hand-built rows: the 2 real observed duplicate-name case
/// (`BootScriptExecutorDxe.efi`) under fabricated `PkgA`/`PkgB` subdirectories,
/// with distinct full embedded paths and base addresses matching
/// [`FIXTURE_ROWS`].
fn hand_built_rows() -> Vec<HandBuiltModuleRow> {
    vec![
        HandBuiltModuleRow {
            embedded_path: format!(
                "{PREFIX}/PkgA/Universal/BootScriptExecutorDxe/DEBUG/BootScriptExecutorDxe.efi"
            ),
            base: 3_739_889_664,
            size: 84_224,
        },
        HandBuiltModuleRow {
            embedded_path: format!(
                "{PREFIX}/PkgB/Universal/BootScriptExecutorDxe/DEBUG/BootScriptExecutorDxe.efi"
            ),
            base: 3_722_764_288,
            size: 84_224,
        },
    ]
}

#[test]
fn resolves_duplicate_names_via_path_suffix_disambiguation_given_full_paths() -> Result<()> {
    let tmp = tempdir()?;
    let root = tmp.path();

    let rows = hand_built_rows();
    let modules: Vec<(String, u64, u64, PathBuf)> = rows
        .iter()
        .map(|row| {
            (
                "BootScriptExecutorDxe.efi".to_string(),
                row.base,
                row.size,
                PathBuf::from(&row.embedded_path),
            )
        })
        .collect();

    // Mirror each row embedded path locally, from "X64/" onward (i.e. the
    // "PkgA/..." / "PkgB/..." part), as a real local file with content unique to
    // that row -- used below to positively confirm which exact local file each
    // module resolved to, not just "a" file.
    let mut expected_local_paths: HashMap<String, PathBuf> = HashMap::new();
    for row in &rows {
        let suffix = row
            .embedded_path
            .rsplit_once("X64/")
            .expect("fixture embedded path contains the X64/ prefix marker")
            .1;
        let local_path = root.join(suffix);
        create_dir_all(
            local_path
                .parent()
                .expect("local fixture path has a parent directory"),
        )?;
        write(&local_path, format!("contents of {suffix}"))?;
        expected_local_paths.insert(row.embedded_path.clone(), local_path);
    }

    let info = UefiOsInfo::resolve(&modules, root)?;
    assert_eq!(info.modules.len(), 2);

    for (name, base, resolved_path) in &info.modules {
        let row = rows
            .iter()
            .find(|row| row.base == *base)
            .expect("resolved module base matches a fixture row");
        let expected = expected_local_paths
            .get(&row.embedded_path)
            .expect("fixture row has a corresponding local fixture file");

        assert_eq!(
            resolved_path, expected,
            "module {name} (base {base:#x}) resolved to the wrong local file; path-suffix \
             disambiguation failed"
        );
    }

    // The actual point of this test, not a vacuous pass: given full,
    // distinguishing embedded paths, the two same-named modules must resolve to
    // two *distinct* local files.
    assert_ne!(
        info.modules[0].2, info.modules[1].2,
        "the two same-named modules must resolve to distinct local files when given distinct \
         full embedded paths"
    );

    Ok(())
}

/// A `tracing_subscriber::fmt::MakeWriter` that captures formatted log output into
/// a shared in-memory buffer, so the tests below can assert on it directly instead
/// of only inferring the warning fired from behavior.
#[derive(Clone, Default)]
struct CapturingWriter(Arc<Mutex<Vec<u8>>>);

impl io::Write for CapturingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .expect("capturing writer mutex not poisoned")
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for CapturingWriter {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

#[test]
fn resolve_falls_open_on_the_real_duplicate_name_case() -> Result<()> {
    let tmp = tempdir()?;
    let root = tmp.path();

    // The full, realistic pipeline: parse_module_list on the confirmed-real
    // fixture, not a hand-built one -- so both BootScriptExecutorDxe.efi entries
    // get parse_module_row own embedded_path (just the bare name, see its doc
    // comment), exactly as real list-modules output would.
    let modules: Vec<_> = parse_module_list(&fixture_attr_value(FIXTURE_ROWS))?
        .into_iter()
        .filter(|(name, ..)| name == "BootScriptExecutorDxe.efi")
        .collect();
    assert_eq!(modules.len(), 2);

    // Two unrelated local directory layouts, both happening to contain a file
    // with the exact bare name "BootScriptExecutorDxe.efi" -- the only kind of
    // local layout that parse_module_list-sourced input can ever match against,
    // since it never has more than a bare name to go on.
    let path_1 = root
        .join("unrelated_layout_one")
        .join("BootScriptExecutorDxe.efi");
    let path_2 = root
        .join("unrelated_layout_two")
        .join("BootScriptExecutorDxe.efi");
    create_dir_all(path_1.parent().expect("has parent"))?;
    create_dir_all(path_2.parent().expect("has parent"))?;
    write(&path_1, b"layout one")?;
    write(&path_2, b"layout two")?;

    let capturing_writer = CapturingWriter::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(capturing_writer.clone())
        .with_ansi(false)
        .with_max_level(tracing::Level::TRACE)
        .finish();

    let info =
        tracing::subscriber::with_default(subscriber, || UefiOsInfo::resolve(&modules, root))?;
    assert_eq!(info.modules.len(), 2);

    // Fail-open (the spec own explicit decision, and -- confirmed live -- the
    // expected outcome for any real duplicate-name module, not a rare edge
    // case): both modules still resolve, to the first candidate in sorted order.
    let mut sorted_candidates = [path_1.clone(), path_2.clone()];
    sorted_candidates.sort();
    let expected_fail_open_path = sorted_candidates[0].clone();

    for (name, _base, resolved_path) in &info.modules {
        assert_eq!(name, "BootScriptExecutorDxe.efi");
        assert_eq!(
            resolved_path, &expected_fail_open_path,
            "expected fail-open fallback to deterministically pick the first (sorted) \
             ambiguous candidate"
        );
    }

    let log_output = String::from_utf8(
        capturing_writer
            .0
            .lock()
            .expect("capturing writer mutex not poisoned")
            .clone(),
    )?;

    assert!(
        log_output.contains("WARN") && log_output.to_lowercase().contains("ambiguous"),
        "expected a WARN-level log message about ambiguous debug info resolution, got: {log_output:?}"
    );

    Ok(())
}
