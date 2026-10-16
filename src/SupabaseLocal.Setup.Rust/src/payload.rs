use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File},
    io::{self, Cursor, Read, Write},
    path::{Path, PathBuf},
};

fn safe_path(root: &Path, name: &str) -> Result<PathBuf> {
    ensure!(
        !name.is_empty() && !name.contains([':', '\\']) && !name.starts_with('/'),
        "Unsafe payload path: {name}"
    );
    ensure!(
        name.split('/').all(|s| !s.is_empty()
            && s != "."
            && s != ".."
            && !s.ends_with(['.', ' '])
            && !s.contains(['<', '>', '|', '?', '*', '\0'])),
        "Unsafe payload path: {name}"
    );
    for part in name.split('/') {
        let stem = part.split('.').next().unwrap().to_ascii_uppercase();
        ensure!(
            !["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str())
                && !(stem.len() == 4
                    && (stem.starts_with("COM") || stem.starts_with("LPT"))
                    && stem.as_bytes()[3].is_ascii_digit()),
            "Reserved Windows path: {name}"
        );
    }
    Ok(root.join(name))
}

fn reject_links(path: &Path) -> Result<()> {
    for parent in path.ancestors() {
        reject_single_link(parent)?;
    }
    Ok(())
}

fn reject_single_link(path: &Path) -> Result<()> {
    if let Ok(meta) = fs::symlink_metadata(path) {
        use std::os::windows::fs::MetadataExt;
        ensure!(
            meta.file_attributes() & 0x400 == 0,
            "Unexpected reparse point: {}",
            path.display()
        );
    }
    Ok(())
}

fn verify(path: &Path, expected: &str) -> Result<()> {
    reject_links(path)?;
    verify_contents(path, expected)
}

fn verify_contents(path: &Path, expected: &str) -> Result<()> {
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    io::copy(&mut file, &mut hash)?;
    ensure!(
        format!("{:x}", hash.finalize()).eq_ignore_ascii_case(expected),
        "Checksum mismatch: {}",
        path.display()
    );
    Ok(())
}

#[derive(Deserialize)]
struct Link {
    #[serde(alias = "Target")]
    target: String,
    #[serde(alias = "Directory")]
    directory: bool,
}

fn restore_links(root: &Path) -> Result<()> {
    let metadata = root.join("links.json");
    if !metadata.exists() {
        return Ok(());
    }
    let links: BTreeMap<String, Link> = serde_json::from_reader(File::open(metadata)?)?;
    for (name, link) in links {
        let alias = safe_path(root, &name)?;
        let target = safe_path(root, &link.target)?;
        ensure!(alias != target, "Runtime link targets itself");
        reject_links(&target)?;
        reject_links(alias.parent().unwrap())?;
        fs::create_dir_all(alias.parent().unwrap())?;
        if link.directory {
            ensure!(target.is_dir(), "Missing runtime directory");
            if fs::symlink_metadata(&alias).is_ok() {
                ensure!(
                    junction::exists(&alias)? && junction::get_target(&alias)? == target,
                    "Runtime junction modified: {name}"
                );
            } else {
                junction::create(&target, &alias)?;
            }
        } else {
            ensure!(target.is_file(), "Missing runtime file");
            if alias.exists() {
                let mut file = File::open(&target)?;
                let mut hash = Sha256::new();
                io::copy(&mut file, &mut hash)?;
                verify(&alias, &format!("{:x}", hash.finalize()))?;
            } else {
                fs::hard_link(target, alias)?;
            }
        }
    }
    Ok(())
}

pub fn install(bytes: &[u8], root: &Path, report: &impl Fn(String)) -> Result<PathBuf> {
    let absolute_root = std::path::absolute(root)?;
    let root = absolute_root.as_path();
    ensure!(
        !bytes.is_empty(),
        "No bundled runtime. Build with scripts/build-installer.ps1."
    );
    reject_links(root)?;
    let version = format!("{:x}", Sha256::digest(bytes));
    let versions = root.join("versions");
    reject_links(&versions)?;
    fs::create_dir_all(&versions)?;
    let destination = versions.join(&version[..16]);
    reject_links(&destination)?;
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
    let files: BTreeMap<String, String> = serde_json::from_reader(
        archive
            .by_name("manifest.json")
            .context("Missing checksum manifest")?,
    )?;
    ensure!(
        files.contains_key("SupabaseLocal.exe") && files.contains_key("SupabaseLocal.App.exe"),
        "Payload is missing its manager or desktop app"
    );
    let mut names = HashSet::new();
    for (name, hash) in &files {
        safe_path(&destination, name)?;
        ensure!(
            names.insert(name.to_ascii_lowercase()),
            "Duplicate Windows payload path"
        );
        ensure!(
            hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()),
            "Invalid checksum"
        );
    }
    if destination.exists() {
        report("Checking installed runtime…".into());
        let mut checked_directories = HashSet::new();
        for (index, (name, hash)) in files.iter().enumerate() {
            let path = safe_path(&destination, name)?;
            for parent in path.parent().unwrap().ancestors() {
                if checked_directories.contains(parent) {
                    break;
                }
                reject_single_link(parent)?;
                checked_directories.insert(parent.to_path_buf());
            }
            reject_single_link(&path)?;
            verify_contents(&path, hash)?;
            if index % 2000 == 0 {
                report(format!(
                    "Verified installed files: {}/{}",
                    index + 1,
                    files.len()
                ));
            }
        }
        report("Checking native runtime links…".into());
        restore_links(&destination.join("runtime"))?;
        return Ok(destination);
    }
    let staging = tempfile::Builder::new()
        .prefix(".staging-")
        .tempdir_in(&versions)?;
    let mut created_directories = HashSet::new();
    let mut buffer = [0u8; 65536];
    for (index, (name, hash)) in files.iter().enumerate() {
        let target = safe_path(staging.path(), name)?;
        let mut entry = archive.by_name(name)?;
        if entry.is_dir() || entry.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
            bail!("Unexpected archive link/directory");
        }
        let parent = target.parent().unwrap();
        if created_directories.insert(parent.to_path_buf()) {
            fs::create_dir_all(parent)?;
        }
        let mut output = File::create(&target)?;
        let mut digest = Sha256::new();
        loop {
            let count = entry.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            output.write_all(&buffer[..count])?;
            digest.update(&buffer[..count]);
        }
        drop(output);
        ensure!(
            format!("{:x}", digest.finalize()).eq_ignore_ascii_case(hash),
            "Checksum mismatch: {name}"
        );
        if index % 200 == 0 {
            report(format!(
                "Installing verified files: {}/{}",
                index + 1,
                files.len()
            ));
        }
    }
    // Restore after the atomic move: junction targets must reference the final location.
    fs::rename(staging.path(), &destination)?;
    report("Restoring native runtime links…".into());
    restore_links(&destination.join("runtime"))?;
    Ok(destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restores_windows_runtime_links_and_rejects_escape() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("package")).unwrap();
        fs::write(root.path().join("package/file.js"), b"runtime").unwrap();
        fs::write(root.path().join("links.json"), r#"{"alias":{"target":"package","directory":true},"file.js":{"target":"package/file.js","directory":false}}"#).unwrap();
        restore_links(root.path()).unwrap();
        restore_links(root.path()).unwrap();
        assert_eq!(
            fs::read(root.path().join("alias/file.js")).unwrap(),
            b"runtime"
        );
        assert_eq!(fs::read(root.path().join("file.js")).unwrap(), b"runtime");
        junction::delete(root.path().join("alias")).unwrap();
        fs::write(
            root.path().join("links.json"),
            r#"{"alias":{"target":"../outside","directory":true}}"#,
        )
        .unwrap();
        assert!(restore_links(root.path()).is_err());
    }
    fn archive(data: &[u8], hash: &str, name: &str) -> Vec<u8> {
        use std::io::Write;
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("manifest.json", options).unwrap();
        let manifest = BTreeMap::from([
            (name, hash),
            ("SupabaseLocal.exe", hash),
            ("SupabaseLocal.App.exe", hash),
        ]);
        writer
            .write_all(&serde_json::to_vec(&manifest).unwrap())
            .unwrap();
        for key in manifest.keys() {
            writer.start_file(*key, options).unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }
    #[test]
    fn install_repeat_tamper_and_data_preservation() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("data")).unwrap();
        fs::write(root.path().join("data/keep"), b"project").unwrap();
        let hash = format!("{:x}", Sha256::digest(b"test"));
        let zip = archive(b"test", &hash, "runtime/test.txt");
        let version = install(&zip, root.path(), &|_| {}).unwrap();
        assert_eq!(install(&zip, root.path(), &|_| {}).unwrap(), version);
        fs::write(version.join("SupabaseLocal.exe"), b"modified").unwrap();
        assert!(install(&zip, root.path(), &|_| {}).is_err());
        assert_eq!(fs::read(root.path().join("data/keep")).unwrap(), b"project");
    }
    #[test]
    fn corrupt_archive_never_commits_a_version() {
        let root = tempfile::tempdir().unwrap();
        let zip = archive(b"test", &"0".repeat(64), "runtime/test.txt");
        assert!(install(&zip, root.path(), &|_| {}).is_err());
        assert_eq!(
            fs::read_dir(root.path().join("versions")).unwrap().count(),
            0
        );
    }
    #[test]
    fn archive_traversal_is_rejected_before_extraction() {
        let root = tempfile::tempdir().unwrap();
        let hash = format!("{:x}", Sha256::digest(b"test"));
        let zip = archive(b"test", &hash, "../escaped.txt");
        assert!(install(&zip, root.path(), &|_| {}).is_err());
        assert_eq!(
            fs::read_dir(root.path().join("versions")).unwrap().count(),
            0
        );
    }
    #[test]
    fn rejects_windows_escape_and_alias_paths() {
        for name in [
            "../x",
            "C:/x",
            "//server/x",
            "a\\..\\x",
            "a:x",
            "a/../x",
            "NUL.txt",
            "a./x",
            "a//x",
        ] {
            assert!(safe_path(Path::new("C:/test"), name).is_err(), "{name}");
        }
        assert!(safe_path(Path::new("C:/test"), "runtime/studio/server.js").is_ok());
    }
}
