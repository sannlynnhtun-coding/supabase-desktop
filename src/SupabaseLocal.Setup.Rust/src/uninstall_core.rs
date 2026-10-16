use anyhow::{Context, Result, ensure};
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
};
use winreg::{
    RegKey,
    enums::{HKEY_CURRENT_USER, KEY_READ},
};

const OWNER: &str = "SupabaseLocal.NativeWindows.v1";
pub const REGISTRY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\SupabaseLocal";

#[derive(Clone)]
pub struct Scope {
    pub root: PathBuf,
    pub programs: PathBuf,
    pub registry: String,
}
impl Scope {
    pub fn current() -> Result<Self> {
        let root =
            PathBuf::from(std::env::var_os("LOCALAPPDATA").context("LOCALAPPDATA unavailable")?)
                .join("SupabaseLocal");
        let folders = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Shell Folders")?;
        let programs: String = folders.get_value("Programs")?;
        Ok(Self {
            root,
            programs: PathBuf::from(programs).join("Supabase Desktop"),
            registry: REGISTRY.into(),
        })
    }
}

pub fn reject_links(path: &Path) -> Result<()> {
    for parent in path.ancestors() {
        match fs::symlink_metadata(parent) {
            Ok(meta) => ensure!(
                meta.file_attributes() & 0x400 == 0,
                "Installation paths cannot pass through reparse points: {}",
                parent.display()
            ),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

// Never traverse reparse points, including broken directory junctions.
pub fn delete_tree(path: &Path, root: &Path) -> Result<()> {
    ensure!(
        path.starts_with(root) && path != root,
        "Removal target escapes installation"
    );
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    if meta.file_attributes() & 0x400 != 0 {
        if meta.file_attributes() & 0x10 != 0 {
            fs::remove_dir(path)?;
        } else {
            fs::remove_file(path)?;
        }
    } else if meta.is_dir() {
        for entry in fs::read_dir(path)? {
            delete_tree(&entry?.path(), root)?;
        }
        fs::remove_dir(path)?;
    } else {
        if meta.permissions().readonly() {
            let mut permissions = meta.permissions();
            permissions.set_readonly(false);
            fs::set_permissions(path, permissions)?;
        }
        fs::remove_file(path)?;
    }
    Ok(())
}

fn exclusive(path: &Path) -> Result<File> {
    reject_links(path)?;
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .share_mode(0)
        .open(path)
        .context("Close Supabase Desktop and its services, then try uninstalling again")
}

pub fn remove(scope: &Scope, remove_data: bool, report: &impl Fn(String)) -> Result<()> {
    let root = std::path::absolute(&scope.root)?;
    ensure!(
        root.parent().is_some() && root.components().count() > 1,
        "Cannot uninstall a drive root"
    );
    reject_links(&root)?;
    reject_links(&scope.programs)?;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu
        .open_subkey_with_flags(&scope.registry, KEY_READ)
        .context("Installation registration is missing")?;
    let owner: String = key.get_value("InstallerOwner")?;
    let location: String = key.get_value("InstallLocation")?;
    ensure!(
        owner == OWNER && location.eq_ignore_ascii_case(&root.to_string_lossy()),
        "Uninstall registration does not belong to this installation"
    );
    drop(key);
    let launcher = exclusive(&root.join("launcher.lock"))?;
    let data = root.join("data");
    reject_links(&data)?;
    let manager = if data.is_dir() {
        Some(exclusive(&data.join("manager.lock"))?)
    } else {
        None
    };
    report("Removing application files…".into());
    delete_tree(&root.join("versions"), &root)?;
    drop(manager);
    if remove_data {
        report("Removing project data…".into());
        delete_tree(&data, &root)?;
    }
    report("Removing Start menu and Installed apps entries…".into());
    for name in [
        "Supabase Desktop.lnk",
        "Uninstall Supabase Desktop.lnk",
        "Supabase Local.lnk",
        "Uninstall Supabase Local.lnk",
    ] {
        let path = scope.programs.join(name);
        match fs::remove_file(path) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.into()),
        }
    }
    if scope.programs.is_dir() && fs::read_dir(&scope.programs)?.next().is_none() {
        fs::remove_dir(&scope.programs)?;
    }
    hkcu.delete_subkey_all(&scope.registry)?;
    drop(launcher);
    fs::remove_file(root.join("launcher.lock"))?;
    if fs::read_dir(&root)?.next().is_none() {
        fs::remove_dir(&root)?;
    }
    Ok(())
}

pub fn validate_worker(directory: &Path) -> Result<()> {
    let directory = std::path::absolute(directory)?;
    let temp = fs::canonicalize(std::env::temp_dir())?;
    let parent = fs::canonicalize(directory.parent().context("Missing worker parent")?)?;
    let name = directory
        .file_name()
        .context("Missing worker directory")?
        .to_string_lossy();
    let suffix = name
        .strip_prefix("SupabaseDesktop-Uninstall-")
        .context("Invalid worker directory")?;
    ensure!(
        parent == temp && suffix.len() == 32 && suffix.bytes().all(|b| b.is_ascii_hexdigit()),
        "Uninstall worker must run from its private temporary directory"
    );
    reject_links(&directory)
}

pub fn wait_for_parent(pid: u32) -> Result<()> {
    ensure!(
        pid != 0 && pid != std::process::id(),
        "Invalid parent process"
    );
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
        fn WaitForSingleObject(handle: isize, milliseconds: u32) -> u32;
        fn CloseHandle(handle: isize) -> i32;
    }
    let handle = unsafe { OpenProcess(0x00100000, 0, pid) };
    if handle == 0 {
        let error = std::io::Error::last_os_error();
        ensure!(
            error.raw_os_error() == Some(87),
            "Could not wait for parent: {error}"
        );
        return Ok(());
    }
    let result = unsafe { WaitForSingleObject(handle, 30_000) };
    unsafe {
        CloseHandle(handle);
    }
    ensure!(
        result == 0,
        "Original uninstaller did not exit within 30 seconds"
    );
    Ok(())
}

pub fn verify_fixture(result: &Path) -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let base = temporary.path();
    let scope = Scope {
        root: base.join("installation with spaces"),
        programs: base.join("programs"),
        registry: format!(
            r"Software\SupabaseDesktopUninstallTests\{}",
            base.file_name().unwrap().to_string_lossy()
        ),
    };
    let fixture = || -> Result<()> {
        fs::create_dir_all(scope.root.join("versions/test"))?;
        fs::create_dir_all(scope.root.join("data"))?;
        fs::create_dir_all(&scope.programs)?;
        fs::write(scope.root.join("versions/test/app.exe"), b"fixture")?;
        fs::write(scope.root.join("data/keep.txt"), b"database fixture")?;
        fs::write(scope.programs.join("Supabase Desktop.lnk"), b"fixture")?;
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(&scope.registry)?;
        key.set_value("InstallerOwner", &OWNER)?;
        key.set_value("InstallLocation", &scope.root.to_string_lossy().as_ref())?;
        Ok(())
    };
    let run = (|| -> Result<()> {
        fixture()?;
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(&scope.registry)?;
        key.set_value("InstallerOwner", &"someone else")?;
        ensure!(
            remove(&scope, false, &|_| {}).is_err(),
            "Wrong ownership was accepted"
        );
        ensure!(
            scope.root.join("versions/test/app.exe").exists(),
            "Ownership rejection modified files"
        );
        key.set_value("InstallerOwner", &OWNER)?;
        drop(key);
        let lock = exclusive(&scope.root.join("launcher.lock"))?;
        ensure!(
            remove(&scope, false, &|_| {}).is_err(),
            "Running launcher was accepted"
        );
        drop(lock);
        let lock = exclusive(&scope.root.join("data/manager.lock"))?;
        ensure!(
            remove(&scope, false, &|_| {}).is_err(),
            "Running manager was accepted"
        );
        drop(lock);
        let outside = base.join("outside");
        fs::create_dir(&outside)?;
        fs::write(outside.join("keep.txt"), b"outside fixture")?;
        junction::create(&outside, scope.root.join("versions/test/link"))?;
        let broken = scope.root.join("versions/test/broken");
        junction::create(&outside, &broken)?;
        // First remove an intact junction and verify its target remains untouched.
        remove(&scope, false, &|_| {})?;
        ensure!(
            !scope.root.join("versions").exists() && !scope.programs.exists(),
            "Application or shortcuts remain"
        );
        ensure!(
            RegKey::predef(HKEY_CURRENT_USER)
                .open_subkey(&scope.registry)
                .is_err(),
            "Registration remains"
        );
        ensure!(
            fs::read(scope.root.join("data/keep.txt"))? == b"database fixture",
            "Project data changed"
        );
        ensure!(
            fs::read(outside.join("keep.txt"))? == b"outside fixture",
            "Junction target changed"
        );
        fixture()?;
        let missing = base.join("missing target");
        fs::create_dir(&missing)?;
        junction::create(&missing, scope.root.join("versions/test/broken"))?;
        fs::remove_dir(&missing)?;
        remove(&scope, true, &|_| {})?;
        ensure!(
            !scope.root.exists(),
            "Explicit data removal left the installation behind"
        );
        fs::create_dir(&scope.root)?;
        junction::create(&outside, scope.root.join("data"))?;
        ensure!(
            reject_links(&scope.root.join("data")).is_err(),
            "Linked data root was accepted"
        );
        fs::remove_dir(scope.root.join("data"))?;
        ensure!(
            delete_tree(&outside, &scope.root).is_err(),
            "Escaping removal target accepted"
        );
        ensure!(
            validate_worker(base).is_err(),
            "Untrusted worker folder accepted"
        );
        Ok(())
    })();
    let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(&scope.registry);
    run?;
    fs::write(
        result,
        serde_json::to_vec_pretty(
            &serde_json::json!({"rustUninstaller":true,"ownershipRejected":true,"runningLauncherRejected":true,"runningManagerRejected":true,"dataPreserved":true,"explicitDataRemoval":true,"junctionTargetsPreserved":true,"brokenJunctionRemoved":true,"unsafePathsRejected":true}),
        )?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uninstall_ownership_locks_links_and_data_choices() {
        let output = tempfile::tempdir().unwrap();
        verify_fixture(&output.path().join("result.json")).unwrap();
    }
    #[test]
    fn worker_path_requires_a_private_direct_child_of_temp() {
        assert!(validate_worker(&std::env::temp_dir()).is_err());
        assert!(
            validate_worker(&std::env::temp_dir().join("SupabaseDesktop-Uninstall-not-a-guid"))
                .is_err()
        );
        let dir = tempfile::Builder::new()
            .prefix("SupabaseDesktop-Uninstall-")
            .rand_bytes(16)
            .tempdir()
            .unwrap();
        // tempfile's random suffix is not our required 32-character hex identifier.
        assert!(validate_worker(dir.path()).is_err());
    }
}
