// Gukhanmun: Experimental source-independent loanword extraction tooling.
// Copyright (C) 2026  Hong Minhee
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use super::{Decisions, Error, Snapshot, audit, canonical_json, extract, sha256};
use serde_json::json;
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

fn resolved(path: &Path) -> Result<PathBuf, Error> {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut out = PathBuf::new();
    for c in absolute.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            c => out.push(c.as_os_str()),
        }
        if fs::symlink_metadata(&out).is_ok() {
            out = out.canonicalize()?;
        }
    }
    Ok(out)
}
// Probe names inside a disposable directory on the destination filesystem.
// This detects its own case/Unicode equivalence rules without guessing tables.
fn absent_alias(a: &Path, b: &Path) -> Result<bool, Error> {
    let (Some(ap), Some(bp), Some(an), Some(bn)) =
        (a.parent(), b.parent(), a.file_name(), b.file_name())
    else {
        return Ok(false);
    };
    if !ap.is_dir() || !bp.is_dir() || !same_file::is_same_file(ap, bp).map_err(|_| Error::Io)? {
        return Ok(false);
    }
    let probe = tempfile::Builder::new()
        .prefix(".gukhanmun-path-")
        .tempdir_in(ap)?;
    let first = probe.path().join(an);
    let second = probe.path().join(bn);
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&first)?;
    Ok(second.exists() && same_file::is_same_file(first, second).map_err(|_| Error::Io)?)
}
/// Checks all output/input and output/output aliases before publishing outputs.
/// Temporary name probes detect the destination filesystem's Unicode equivalence.
/// Resolves relative paths and existing symlink parents; existing hardlinks are
/// compared by file identity. ASCII case-only path differences are conservatively
/// rejected even on case-sensitive filesystems. Output symlinks are rejected.
pub fn preflight_paths(inputs: &[&Path], outputs: &[&Path]) -> Result<(), Error> {
    preflight(inputs, outputs, true)
}
fn preflight(inputs: &[&Path], outputs: &[&Path], probe: bool) -> Result<(), Error> {
    let inputs = inputs
        .iter()
        .map(|p| resolved(p))
        .collect::<Result<Vec<_>, _>>()?;
    let mut accepted = Vec::<PathBuf>::new();
    for p in outputs {
        if fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(Error::PathCollision);
        }
        let r = resolved(p)?;
        for prior in inputs.iter().chain(&accepted) {
            if r.as_os_str()
                .as_encoded_bytes()
                .eq_ignore_ascii_case(prior.as_os_str().as_encoded_bytes())
                || (r.exists()
                    && prior.exists()
                    && same_file::is_same_file(&r, prior).map_err(|_| Error::Io)?)
                || (probe && !r.exists() && !prior.exists() && absent_alias(&r, prior)?)
            {
                return Err(Error::PathCollision);
            }
        }
        accepted.push(r);
    }
    Ok(())
}
fn atomic(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let existing_permissions = match fs::metadata(path) {
        Ok(metadata) => Some(metadata.permissions()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => return Err(Error::Io),
    };
    let mut builder = tempfile::Builder::new();
    builder.prefix(".gukhanmun-");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // Use the destination's mode at creation too, so a private output is
        // never staged with broader permissions. New files respect the umask.
        builder.permissions(
            existing_permissions
                .clone()
                .unwrap_or_else(|| fs::Permissions::from_mode(0o666)),
        );
    }
    let mut temp = builder.tempfile_in(parent)?;
    if let Some(permissions) = existing_permissions {
        // Creation applies the current umask; restore an existing mode before
        // writing, matching ordinary replacement even under a stricter umask.
        temp.as_file().set_permissions(permissions)?;
    }
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|_| Error::Io)?;
    Ok(())
}
fn read(path: &Path) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(256 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 256 * 1024 * 1024 {
        return Err(Error::Invalid("offline input byte limit"));
    }
    Ok(bytes)
}
// Compare generated artifacts without applying the independent offline input cap.
// Read at most the expected length plus one byte, using fixed-size scratch space.
fn matches_artifact(path: &Path, expected: &[u8]) -> Result<bool, Error> {
    let mut file = fs::File::open(path)?;
    let mut buffer = [0; 8192];
    for chunk in expected.chunks(buffer.len()) {
        match file.read_exact(&mut buffer[..chunk.len()]) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(false),
            Err(_) => return Err(Error::Io),
        }
        if &buffer[..chunk.len()] != chunk {
            return Ok(false);
        }
    }
    Ok(file.read(&mut buffer[..1])? == 0)
}
/// Explicit paths for one extraction transaction; none are implicit repository pins.
#[derive(Clone, Debug)]
pub struct ExtractionPaths {
    /// Validated self-contained snapshot input.
    pub snapshot: PathBuf,
    /// Optional editorial decisions input.
    pub decisions: Option<PathBuf>,
    /// Optional previous snapshot, for corrections/deletions reporting.
    pub prior_snapshot: Option<PathBuf>,
    /// Successful TSV destination.
    pub tsv: PathBuf,
    /// Successful report destination, with input/decisions/TSV checksums.
    pub report: PathBuf,
    /// Optional separate failure-only diagnostic destination.
    pub diagnostic: Option<PathBuf>,
}
/// Extracts or checks artifacts after preflighting every destination, diagnostics included.
/// Check mode never writes. TSV publishes before its independently hashed report;
/// a report failure returns `IncompletePair`, and check rejects that stale pair.
/// Audit-only mode writes only a failure-distinguished audit document to `report`.
/// Successful extraction removes any previous failure diagnostic after publishing
/// both artifacts; a cleanup failure returns `Io`. Check and audit-only modes
/// preserve that diagnostic.
pub fn process_files(paths: &ExtractionPaths, check: bool, audit_only: bool) -> Result<(), Error> {
    let mut inputs = vec![paths.snapshot.as_path()];
    inputs.extend(paths.decisions.as_deref());
    inputs.extend(paths.prior_snapshot.as_deref());
    let mut outputs = vec![paths.tsv.as_path(), paths.report.as_path()];
    outputs.extend(paths.diagnostic.as_deref());
    preflight(&inputs, &outputs, !check)?;
    let snapshot = Snapshot::parse(&read(&paths.snapshot)?)?;
    let decisions = paths
        .decisions
        .as_ref()
        .map(|p| read(p).and_then(|b| Decisions::parse(&b)))
        .transpose()?
        .unwrap_or_default();
    let prior = paths
        .prior_snapshot
        .as_ref()
        .map(|p| read(p).and_then(|b| Snapshot::parse(&b)))
        .transpose()?;
    let current_audit = audit(&snapshot, &decisions);
    let previous_audit = prior.as_ref().map(|s| audit(s, &decisions));
    let current_keys = current_audit
        .candidates
        .keys()
        .collect::<std::collections::BTreeSet<_>>();
    let old_keys = previous_audit
        .as_ref()
        .map(|a| {
            a.candidates
                .keys()
                .collect::<std::collections::BTreeSet<_>>()
        })
        .unwrap_or_default();
    let delta = json!({"records":prior.as_ref().map(|p|snapshot.diff(p)),"keys_added":current_keys.difference(&old_keys).collect::<Vec<_>>(),"keys_removed":old_keys.difference(&current_keys).collect::<Vec<_>>(),"keys_changed":current_keys.intersection(&old_keys).filter(|k|previous_audit.as_ref().unwrap().candidates.get(**k)!=current_audit.candidates.get(**k)).collect::<Vec<_>>()});
    let diagnostic = canonical_json(
        &json!({"format_version":1,"status":"audit-only","snapshot_sha256":sha256(&snapshot.bytes()),"decisions_sha256":decisions.fingerprint(),"audit":current_audit,"delta":delta}),
    );
    if audit_only {
        if check {
            if !matches_artifact(&paths.report, &diagnostic)? {
                return Err(Error::Stale);
            }
        } else {
            atomic(&paths.report, &diagnostic)?;
        }
        return Ok(());
    }
    let extraction = match extract(&snapshot, &decisions) {
        Ok(v) => v,
        Err(e) => {
            if !check && let Some(p) = &paths.diagnostic {
                atomic(p, &diagnostic)?;
            }
            return Err(e);
        }
    };
    let report = canonical_json(
        &json!({"format_version":1,"status":"success","snapshot_sha256":sha256(&snapshot.bytes()),"decisions_sha256":decisions.fingerprint(),"tsv_sha256":sha256(&extraction.tsv),"audit":extraction.audit,"delta":delta}),
    );
    if check {
        if !matches_artifact(&paths.tsv, &extraction.tsv)?
            || !matches_artifact(&paths.report, &report)?
        {
            return Err(Error::Stale);
        }
    } else {
        atomic(&paths.tsv, &extraction.tsv)?;
        atomic(&paths.report, &report).map_err(|_| Error::IncompletePair)?;
        if let Some(path) = &paths.diagnostic {
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
    }
    Ok(())
}
/// Rejects collection destinations inside any ancestor Git checkout.
/// Collection uses an external staging destination; the updater publishes verified data.
pub fn collect_destination(path: &Path) -> Result<(), Error> {
    let path = resolved(path)?;
    if path.ancestors().any(|p| p.join(".git").exists()) {
        return Err(Error::Invalid(
            "raw collection destination must be outside a checkout",
        ));
    }
    Ok(())
}
/// Atomically publishes a validated self-contained external snapshot, or compares
/// record content in check mode without writing or comparing collection timestamps.
pub fn publish_snapshot(path: &Path, snapshot: &Snapshot, check: bool) -> Result<(), Error> {
    collect_destination(path)?;
    preflight(&[], &[path], !check)?;
    Snapshot::parse(&snapshot.bytes())?;
    if check {
        let existing = Snapshot::parse(&read(path)?)?;
        if existing.records != snapshot.records {
            return Err(Error::Stale);
        }
    } else {
        atomic(path, &snapshot.bytes())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_comparison_has_no_offline_input_cap() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("large-report.json");
        let expected = vec![0; 256 * 1024 * 1024 + 1];
        // A sparse file exercises the old 256 MiB boundary without writing it all.
        fs::File::create(&path)
            .unwrap()
            .set_len(expected.len() as u64)
            .unwrap();
        assert!(matches_artifact(&path, &expected).unwrap());
        assert!(read(&path).is_err());
    }

    #[test]
    fn artifact_comparison_rejects_mismatches_and_length_changes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.json");
        for content in [b"".as_slice(), b"ab", b"abc", b"abcd", b"axc"] {
            fs::write(&path, content).unwrap();
            assert_eq!(matches_artifact(&path, b"abc").unwrap(), content == b"abc");
        }
    }
}
