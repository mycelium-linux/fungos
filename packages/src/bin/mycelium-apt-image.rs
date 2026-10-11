//! Clone only a retained private package-qualification disk for signed APT proof.
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
fn run(program: &str, args: &[&str]) -> Result<String> {
    let out = Command::new(program).args(args).output()?;
    if !out.status.success() {
        return Err(format!(
            "{program}: {}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
        .into());
    }
    Ok(String::from_utf8(out.stdout)?)
}
fn text(path: &Path) -> Result<&str> {
    path.to_str().ok_or_else(|| "non UTF8 path".into())
}
fn put(root: &Path, path: &str, content: &str) -> Result<()> {
    let path = root.join(path);
    fs::create_dir_all(path.parent().ok_or("missing parent")?)?;
    fs::write(path, content)?;
    Ok(())
}
fn regular(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.is_file())
}
fn with_unibus_probe(old: &str) -> Result<String> {
    // The prerequisite owner proof generated this JSON. Preserve its existing
    // bindings verbatim; the real owner engine parses/validates the result.
    let old = old.trim();
    if old.len() > 1024 * 1024
        || !old.starts_with('{')
        || !old.ends_with('}')
        || old.contains("\"unibus-core\"")
    {
        return Err(
            "private owner binding must be bounded and lack an existing Unibus entry".into(),
        );
    }
    let body = old[1..old.len() - 1].trim();
    let separator = if body.is_empty() { "" } else { "," };
    Ok(format!("{{{body}{separator}\"unibus-core\":{{\"unit\":\"unibus-router.service\",\"timeout_secs\":10,\"readiness\":{{\"kind\":\"unix\",\"path\":\"/run/unibus/health.sock\",\"request\":\"health\\n\",\"expect_prefix\":\"READY unibus\\n\"}}}}}}"))
}
fn main() -> Result<()> {
    let mut args: Vec<_> = std::env::args().skip(1).collect();
    let unibus = args.first().is_some_and(|arg| arg == "--unibus");
    if unibus {
        args.remove(0);
    }
    if args.len() != 4 {
        return Err("usage: mycelium-apt-image [--unibus] PRIVATE_QUALIFIED_RAW_DISK NEW_OUTPUT_DIR SIGNED_REPOSITORY PROOF_BINARY".into());
    }
    if run("id", &["-u"])?.trim() != "0" {
        return Err("root required for private disposable image assembly".into());
    }
    for i in [0, 3] {
        if !regular(Path::new(&args[i])) {
            return Err("input must be regular nonsymlink file, not a device".into());
        }
    }
    let repository = Path::new(&args[2]);
    for file in [
        "SHA256SUMS",
        "dists/testing/InRelease",
        "fungos-archive-keyring.gpg",
    ] {
        if !regular(&repository.join(file)) {
            return Err("missing genuine signed repository".into());
        }
    }
    let verify = Command::new("sha256sum")
        .args(["--check", "SHA256SUMS"])
        .current_dir(repository)
        .output()?;
    if !verify.status.success() {
        return Err("signed repository inventory mismatch".into());
    }
    let out = PathBuf::from(&args[1]);
    fs::create_dir(&out)?;
    fs::set_permissions(&out, fs::Permissions::from_mode(0o700))?;
    let image = out.join("disk.raw");
    run(
        "cp",
        &["--reflink=auto", "--sparse=always", &args[0], text(&image)?],
    )?;
    let device = run(
        "losetup",
        &["--find", "--show", "--partscan", text(&image)?],
    )?
    .trim()
    .to_owned();
    if !device.starts_with("/dev/loop") || !device[9..].bytes().all(|b| b.is_ascii_digit()) {
        return Err("unexpected allocated loop device".into());
    }
    let root = out.join("mount");
    fs::create_dir(&root)?;
    if let Err(error) = run("mount", &[&format!("{device}p2"), text(&root)?]) {
        run("losetup", &["-d", &device])?;
        return Err(error);
    }
    let prepared = (|| -> Result<()> {
        for relative in ["", "etc", "etc/ssh"] {
            let metadata = fs::symlink_metadata(root.join(relative))?;
            if !metadata.is_dir()
                || !safe_directory_owner(metadata.uid(), metadata.gid(), metadata.mode())
            {
                return Err(
                    format!("private fixture root ownership/mode invalid: /{relative}").into(),
                );
            }
        }
        if !root.join("var/lib/fungos-mycelium-package-proof").is_file()
            || !root.join("usr/bin/mycelium").is_file()
            || root.join("var/lib/fungos-mycelium-apt-proof").exists()
        {
            return Err(
                "not the completed private qualification disk; refuses fleet/unqualified disk"
                    .into(),
            );
        }
        run(
            "chroot",
            &[
                text(&root)?,
                "systemctl",
                "disable",
                "fungos-mycelium-package-proof.service",
            ],
        )?;
        // No package is fabricated or installed here; the runtime engine owns
        // signed admission, staging, installation and recovery in the guest.
        let qa = root.join("opt/fungos-mycelium-apt-qa");
        fs::create_dir(&qa)?;
        run("cp", &["-a", &args[2], text(&qa.join("repository"))?])?;
        fs::copy(&args[3], qa.join("proof"))?;
        fs::set_permissions(qa.join("proof"), fs::Permissions::from_mode(0o755))?;
        if unibus {
            // Only this private clone receives a probe grant. No app grants,
            // LAN listener or production authority are introduced.
            if !regular(&root.join("usr/bin/unibus-router")) {
                return Err("Unibus mode requires installed owner router".into());
            }
            use std::io::Read;
            let mut entropy = [0; 32];
            fs::File::open("/dev/urandom")?.read_exact(&mut entropy)?;
            let token: String = entropy.iter().map(|byte| format!("{byte:02x}")).collect();
            put(&root, "opt/fungos-mycelium-apt-qa/health-token", &token)?;
            fs::set_permissions(qa.join("health-token"), fs::Permissions::from_mode(0o600))?;
            put(
                &root,
                "etc/unibus/hub.json",
                r#"{"installation_id":"qa8-update","listen":"127.0.0.1:18790","allow_insecure_lan":false,"routes":[],"nodes":[{"producer_id":"health","token_file":"/run/credentials/unibus-router.service/health-token","zones":["health"],"capabilities":["health.probe"]}]}"#,
            )?;
            put(
                &root,
                "opt/fungos-mycelium-apt-qa/health-client.json",
                r#"{"installation_id":"qa8-update","producer_id":"health","hub_endpoint":"127.0.0.1:18790","token_file":"/run/credentials/unibus-router.service/health-token","zones":["health"],"schema_version":2,"read_timeout_ms":100}"#,
            )?;
            put(&root, "etc/systemd/system/unibus-router.service.d/90-private-apt-health.conf", "[Service]\nLoadCredential=health-token:/opt/fungos-mycelium-apt-qa/health-token\nLoadCredential=health-client:/opt/fungos-mycelium-apt-qa/health-client.json\nEnvironment=UNIBUS_HEALTH_SOCKET=/run/unibus/health.sock\nEnvironment=UNIBUS_HEALTH_CLIENT=/run/credentials/unibus-router.service/health-client\n")?;
            let policy = root.join("var/lib/mycelium/software-services.json");
            let old = fs::read_to_string(&policy)?;
            fs::write(&policy, with_unibus_probe(&old)?)?;
            fs::set_permissions(&policy, fs::Permissions::from_mode(0o600))?;
        }
        for original in ["etc/apt/sources.list", "etc/apt/sources.list.d"] {
            let path = root.join(original);
            if path.exists() {
                fs::rename(
                    path,
                    root.join(format!("{original}.before-mycelium-apt-proof")),
                )?;
            }
        }
        fs::create_dir(root.join("etc/apt/sources.list.d"))?;
        put(&root, "etc/apt/sources.list", "")?;
        put(&root, "etc/apt/sources.list.d/fungos-test.sources", "Types: deb\nURIs: file:/opt/fungos-mycelium-apt-qa/repository\nSuites: testing\nComponents: main\nArchitectures: amd64\nSigned-By: /opt/fungos-mycelium-apt-qa/repository/fungos-archive-keyring.gpg\nCheck-Valid-Until: yes\n")?;
        // Ignore stale lists from the earlier OS fixture; the first real update
        // authenticates only this test authority. Private identity stays intact.
        let lists = root.join("var/lib/apt/lists");
        if lists.exists() {
            fs::rename(
                &lists,
                root.join("var/lib/apt/lists.before-mycelium-apt-proof"),
            )?;
        }
        fs::create_dir(&lists)?;
        put(&root, "usr/lib/systemd/system/fungos-mycelium-apt-proof.service", "[Unit]\nDescription=Private signed Mycelium APT qualification\nAfter=mycelium.service network-online.target\nWants=mycelium.service network-online.target\nConditionVirtualization=yes\n\n[Service]\nType=oneshot\nExecStart=/opt/fungos-mycelium-apt-qa/proof\nTimeoutStartSec=360\nUMask=0077\nStandardOutput=journal\nStandardError=journal\n\n[Install]\nWantedBy=multi-user.target\n")?;
        if unibus {
            put(&root, "etc/systemd/system/fungos-mycelium-apt-proof.service.d/10-unibus.conf", "[Unit]\nAfter=unibus-router.service\nWants=unibus-router.service\n[Service]\nExecStart=\nExecStart=/opt/fungos-mycelium-apt-qa/proof --unibus-only\n")?;
        }
        run(
            "chroot",
            &[
                text(&root)?,
                "systemctl",
                "enable",
                "fungos-mycelium-apt-proof.service",
            ],
        )?;
        Ok(())
    })();
    let unmounted = run("umount", &[text(&root)?]);
    if unmounted.is_ok() {
        run("losetup", &["-d", &device])?;
    }
    prepared?;
    unmounted?;
    println!("assembled private signed APT image: {}", image.display());
    Ok(())
}
fn safe_directory_owner(uid: u32, gid: u32, mode: u32) -> bool {
    uid == 0 && gid == 0 && mode & 0o022 == 0
}
#[cfg(test)]
mod tests {
    #[test]
    fn adds_private_probe_without_overwriting_owner_binding() {
        let empty = super::with_unibus_probe("{}\n").unwrap();
        assert!(empty.starts_with("{\"unibus-core\":"));
        assert!(empty.ends_with("}}}"));
        let owner = "{\"mycelium\":{\"timeout_secs\":30}}";
        assert!(super::with_unibus_probe(owner)
            .unwrap()
            .starts_with("{\"mycelium\":{\"timeout_secs\":30},\"unibus-core\":"));
        assert!(super::with_unibus_probe(&empty).is_err());
        assert!(super::with_unibus_probe("[]").is_err());
        assert!(super::with_unibus_probe(&format!("{{{}}}", " ".repeat(1024 * 1024))).is_err());
    }
    #[test]
    fn rejects_builder_owned_or_writable_ancestors() {
        assert!(super::safe_directory_owner(0, 0, 0o755));
        assert!(!super::safe_directory_owner(1000, 1000, 0o755));
        assert!(!super::safe_directory_owner(0, 0, 0o775));
    }
    #[test]
    fn refuses_devices() {
        assert!(!super::regular(std::path::Path::new("/dev/null")));
    }
}
