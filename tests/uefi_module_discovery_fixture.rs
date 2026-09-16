// Copyright (C) 2024 Intel Corporation
// SPDX-License-Identifier: Apache-2.0

//! End-to-end, offline test of UEFI module discovery's milestone-scope steps 1-2
//! (`tsffs::uefi::{parse_module_list, UefiOsInfo}`, UCOV-M2) against a synthetic
//! fixture, entirely without a live Simics session -- mirroring
//! `tests/dwarf_fixture.rs`'s pattern on the sibling DWARF milestone branch
//! (`feat/dwarf-source-coverage-ucov-m1`).
//!
//! This lives under `tests/` (an integration test / separate cargo crate) rather
//! than as a `#[cfg(test)]` module inside `src/uefi/mod.rs` for the same reason as
//! that file: this crate's `[lib]` section sets `test = false`, which disables the
//! implicit unit-test harness for the library target, so `#[cfg(test)]` code
//! inside `src/` is never compiled by `cargo test` for this crate. Integration
//! tests under `tests/` are a separate cargo target and unaffected by that
//! setting. Being a separate crate also means this file only sees `tsffs`'s `pub`
//! API -- `src/lib.rs` widens `uefi` to `pub` (from `pub(crate)`) for exactly this
//! reason, same as `dwarf`/`source_cov` on the DWARF branch.
//!
//! # No live Simics session, no real `list-modules` output was used
//!
//! There is no BIOS image, boot, or live Simics session available in this offline
//! environment, so milestone-scope step 1 (the `list-modules` query mechanism)
//! reduces to testing `parse_module_list` against a *hand-constructed* fake
//! `AttrValueType` shape rather than a real one -- see `src/uefi/mod.rs`'s module
//! doc comment ("Assumed shape of `list-modules`' return value") for exactly what
//! shape is assumed and why, and for why `AttrValueType` (not `AttrValue`) is the
//! safe, FFI-free type to hand-construct offline.
//!
//! # Fixture
//!
//! The fixture models 7 modules using realistic-looking embedded build-machine
//! paths (the same `SimicsOpenBoardPkg`/`BoardX58Ich10`/`RELEASE_GCC5` structure
//! observed in a prior investigation's real, live 2026-03-12 tracker dump),
//! including the 3 real observed duplicate-name cases called out in the UCOV-M2
//! spec -- `AcpiVTD.efi`, `MicrocodeUtilityDxe.efi`, `SataController.efi` -- each
//! appearing twice with distinct embedded paths (fabricated `PkgA`/`PkgB`
//! subdirectories, per the spec's own suggestion) and distinct base addresses.

use std::{
    collections::{BTreeMap, HashMap},
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

/// One row of the fixture: an embedded build-machine path plus its base/size, in
/// the shape `parse_module_list` is documented (see `src/uefi/mod.rs`) to expect.
struct FixtureRow {
    embedded_path: String,
    base: u64,
    size: u64,
}

/// The fixture's common embedded build-machine path prefix, matching the real
/// structure observed in the prior investigation's live tracker dump.
const PREFIX: &str =
    "/home/robertgu/mydev/simics-build/Build/SimicsOpenBoardPkg/BoardX58Ich10/RELEASE_GCC5/IA32";

/// Build the fixture's 7 rows: one unique module (`PeiCore.efi`) plus the 3 real
/// observed duplicate-name cases, each appearing twice under fabricated `PkgA`/
/// `PkgB` package subdirectories with distinct embedded paths and base addresses.
fn fixture_rows() -> Vec<FixtureRow> {
    vec![
        FixtureRow {
            embedded_path: format!("{PREFIX}/MdeModulePkg/Core/Pei/PeiMain/DEBUG/PeiCore.efi"),
            base: 0x0000_0000_0082_0000,
            size: 0x9000,
        },
        FixtureRow {
            embedded_path: format!("{PREFIX}/PkgA/Feature/AcpiVTD/DEBUG/AcpiVTD.efi"),
            base: 0x0000_0000_0700_0000,
            size: 0x4000,
        },
        FixtureRow {
            embedded_path: format!("{PREFIX}/PkgB/Feature/AcpiVTD/DEBUG/AcpiVTD.efi"),
            base: 0x0000_0000_0710_0000,
            size: 0x4200,
        },
        FixtureRow {
            embedded_path: format!(
                "{PREFIX}/PkgA/Universal/MicrocodeUtilityDxe/DEBUG/MicrocodeUtilityDxe.efi"
            ),
            base: 0x0000_0000_0720_0000,
            size: 0x3000,
        },
        FixtureRow {
            embedded_path: format!(
                "{PREFIX}/PkgB/Universal/MicrocodeUtilityDxe/DEBUG/MicrocodeUtilityDxe.efi"
            ),
            base: 0x0000_0000_0730_0000,
            size: 0x3100,
        },
        FixtureRow {
            embedded_path: format!(
                "{PREFIX}/PkgA/Bus/Pci/SataControllerDxe/DEBUG/SataController.efi"
            ),
            base: 0x0000_0000_0740_0000,
            size: 0x5000,
        },
        FixtureRow {
            embedded_path: format!(
                "{PREFIX}/PkgB/Bus/Pci/SataControllerDxe/DEBUG/SataController.efi"
            ),
            base: 0x0000_0000_0750_0000,
            size: 0x5100,
        },
    ]
}

/// Build the fake `AttrValueType` shape `list-modules` is assumed to return (see
/// `src/uefi/mod.rs`'s module doc comment) for a set of fixture rows: a `List` of
/// `Dict`s, each keyed by `"name"` (the full embedded path, as a `String`),
/// `"base"`, and `"size"` (both `Unsigned`).
fn fixture_attr_value(rows: &[FixtureRow]) -> AttrValueType {
    AttrValueType::List(
        rows.iter()
            .map(|row| {
                let mut fields = BTreeMap::new();
                fields.insert(
                    AttrValueType::String("name".to_string()),
                    AttrValueType::String(row.embedded_path.clone()),
                );
                fields.insert(
                    AttrValueType::String("base".to_string()),
                    AttrValueType::Unsigned(row.base),
                );
                fields.insert(
                    AttrValueType::String("size".to_string()),
                    AttrValueType::Unsigned(row.size),
                );
                AttrValueType::Dict(fields)
            })
            .collect(),
    )
}

#[test]
fn parses_synthetic_module_list_with_duplicate_names() -> Result<()> {
    let rows = fixture_rows();
    let value = fixture_attr_value(&rows);

    let parsed = parse_module_list(&value)?;
    assert_eq!(parsed.len(), rows.len());

    for (row, (name, base, size, embedded_path)) in rows.iter().zip(parsed.iter()) {
        let expected_name = PathBuf::from(&row.embedded_path)
            .file_name()
            .expect("fixture embedded path has a file name")
            .to_str()
            .expect("fixture file name is valid UTF-8")
            .to_string();

        assert_eq!(name, &expected_name);
        assert_eq!(*base, row.base);
        assert_eq!(*size, row.size);
        assert_eq!(embedded_path, &PathBuf::from(&row.embedded_path));
    }

    // The 3 real observed duplicate-name cases: each must appear exactly twice,
    // with distinct embedded paths and distinct base addresses (not accidentally
    // collapsed/deduplicated by the parser, and not confused with each other).
    for dup_name in ["AcpiVTD.efi", "MicrocodeUtilityDxe.efi", "SataController.efi"] {
        let matches: Vec<_> = parsed.iter().filter(|(name, ..)| name == dup_name).collect();
        assert_eq!(
            matches.len(),
            2,
            "expected exactly 2 parsed entries for duplicate-name module {dup_name}"
        );
        assert_ne!(
            matches[0].3, matches[1].3,
            "the two {dup_name} entries must have distinct embedded paths"
        );
        assert_ne!(
            matches[0].1, matches[1].1,
            "the two {dup_name} entries must have distinct base addresses"
        );
    }

    Ok(())
}

#[test]
fn resolves_duplicate_names_via_path_suffix_disambiguation() -> Result<()> {
    let tmp = tempdir()?;
    let root = tmp.path();

    let rows = fixture_rows();
    let modules = parse_module_list(&fixture_attr_value(&rows))?
        .into_iter()
        .filter(|(name, ..)| name == "AcpiVTD.efi" || name == "MicrocodeUtilityDxe.efi")
        .collect::<Vec<_>>();
    assert_eq!(modules.len(), 4);

    // Mirror each relevant fixture row's embedded path locally, from "IA32/"
    // onward (i.e. the "PkgA/..." / "PkgB/..." part), as a real local file with
    // content unique to that row -- used below to positively confirm which exact
    // local file each module resolved to, not just "a" file.
    let mut expected_local_paths: HashMap<String, PathBuf> = HashMap::new();
    for row in rows
        .iter()
        .filter(|row| modules.iter().any(|(_, base, ..)| *base == row.base))
    {
        let suffix = row
            .embedded_path
            .rsplit_once("IA32/")
            .expect("fixture embedded path contains the IA32/ prefix marker")
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
    assert_eq!(info.modules.len(), 4);

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
            "module {name} (base {base:#x}) resolved to the wrong local file; \
             path-suffix disambiguation failed"
        );
    }

    // The actual point of this test, not a vacuous pass: the two AcpiVTD.efi
    // entries must resolve to two *distinct* local files (and likewise for
    // MicrocodeUtilityDxe.efi) -- a resolver that just grabbed "any" file matching
    // the bare name would pass the per-module checks above only by accident, but
    // could not pass this.
    for dup_name in ["AcpiVTD.efi", "MicrocodeUtilityDxe.efi"] {
        let resolved: Vec<&PathBuf> = info
            .modules
            .iter()
            .filter(|(name, ..)| name == dup_name)
            .map(|(_, _, path)| path)
            .collect();
        assert_eq!(resolved.len(), 2);
        assert_ne!(
            resolved[0], resolved[1],
            "the two {dup_name} modules must resolve to distinct local files"
        );
    }

    Ok(())
}

/// A `tracing_subscriber::fmt::MakeWriter` that captures formatted log output into
/// a shared in-memory buffer, so the test below can assert on it directly instead
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
fn falls_back_to_stem_match_and_warns_on_ambiguity_when_suffix_match_fails() -> Result<()> {
    let tmp = tempdir()?;
    let root = tmp.path();

    let rows = fixture_rows();
    let modules = parse_module_list(&fixture_attr_value(&rows))?
        .into_iter()
        .filter(|(name, ..)| name == "SataController.efi")
        .collect::<Vec<_>>();
    assert_eq!(modules.len(), 2);

    // Deliberately unrelated local directory layouts: neither shares any path
    // component with the fixture's fabricated ".../PkgA|PkgB/Bus/Pci/
    // SataControllerDxe/DEBUG/" embedded-path tail beyond the bare file name
    // itself. Path-suffix matching therefore finds the bare file name
    // "SataController.efi" as a candidate suffix, but with *two* different local
    // files sharing it -- an ambiguous, not unique, match -- so per
    // `PathSuffixIndex::lookup_components_unambiguous`'s contract it must return
    // no match at all, forcing both modules through the bare-stem fallback path.
    let path_1 = root.join("unrelated_layout_one").join("SataController.efi");
    let path_2 = root.join("unrelated_layout_two").join("SataController.efi");
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

    let info = tracing::subscriber::with_default(subscriber, || UefiOsInfo::resolve(&modules, root))?;
    assert_eq!(info.modules.len(), 2);

    // Fail-open (the spec's own explicit decision): both modules still resolve --
    // not an error, not a dropped module -- to the first candidate in sorted
    // order. Both modules share the exact same ambiguous candidate set (the same
    // two unrelated local files), so both must fail open to the exact same
    // resolved path.
    let mut sorted_candidates = [path_1.clone(), path_2.clone()];
    sorted_candidates.sort();
    let expected_fail_open_path = sorted_candidates[0].clone();

    for (name, _base, resolved_path) in &info.modules {
        assert_eq!(name, "SataController.efi");
        assert_eq!(
            resolved_path, &expected_fail_open_path,
            "expected fail-open fallback to deterministically pick the first \
             (sorted) ambiguous candidate"
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
