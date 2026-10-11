# Installer qualification

An installer login prompt is an OS boot result, not an operational fungOS release
result. Qualification must identify the exact ISO digest, package inventory and
component manifest. Changing the Debian snapshot or component bundle invalidates
the prior final-image qualification.

## Required final-image evidence

| Gate | Evidence |
|---|---|
| Public interactive entry | Boot the unmodified ISO, select Install, complete the account and disk-confirmation flow on a new disposable disk. |
| BIOS and UEFI installation | Run `qemu-proof.sh` with each firmware mode; preserve input manifest, installer log and installed-disk-only boot log. |
| First boot | Verify generated machine ID and SSH host key, wired networking, failed units and expected component services. |
| Enrollment and SSH | Deliver a private short-lived test claim separately from the public ISO; authenticate using the isolated test authority, verify certificate-only SSH and rejection without a grant. |
| Signed update and rollback | Install genuine component packages, reject tampered/untrusted input, activate a signed update and verify rollback using the owner updater. |
| Reboot persistence | Boot from installed disk again; verify peer identity, SSH host key and installed component versions remain correct. |

Use only fresh harness-created disks. No physical disk, production identity,
private key, password or claim belongs in public media. Retain source/license
inputs for genuine component packages; fixtures cannot prove the runtime gates.
Secure Boot is separate from ordinary UEFI qualification.

## Combined headless-edge install (2026-10-11 UTC)

QA8 includes genuine Mycelium and Unibus owner packages with APT-only ownership.
It was built from fungOS source `601a03a564b55767162815f0161e905e5572e90d`,
trixie snapshot `20261009T000000Z`, and verified Debian 13.1.0 installer input.

- ISO SHA-256: `521bbb5672d4a098a03c9e92474abca2852104f972a611cc656e77b09e15859c`.
- Component manifest SHA-256: `8561f4ca0086926557865f3bd6eb2e81dd40fb3ab07eb966c2861fdf107b938c`.
- Mycelium archive SHA-256: `c6b957afe046cec084b89383cabf1b69b7ad23ae5baf8711667659c03fffa023`.
- Unibus owner inputs: [exact package/source evidence](../release/UNIBUS-NOTICES.md).

Both fresh `qemu-proof.sh` runs exited zero: profile convergence emitted
`FUNGOS_INSTALL_COMPLETE`, then each disk-only boot reached `fungos login:`.
The UEFI run used a private OVMF variables file and booted the installed GPT/ESP
loader. No physical disks or preexisting guests were changed.

| Retained evidence | SHA-256 |
|---|---|
| `bios-install.log` | `3c2f540b2cf8da5378357edf995d550f6c4a3e955de34461ed2a7ffed78e9aa8` |
| `bios-boot.log` | `3a335f87a17e6a096acddd7fb29f517e12af35da3d27c2e9376638f0f8ad779b` |
| `uefi-install.log` | `9141f62bd7924a72d1b45176738d6037a4a1a5031dcd3373d8d6e58be352c229` |
| `uefi-boot.log` | `92d7bceee9b23542ea74a45a33c22b8f26d752b5f1a7b496520e3207e0ca34e7` |

Logs and artifacts are retained on Agora under
`/home/ajmwagar/.cache/fungos-headless-qa8.fTarSb/`. Temporary test disks are
under `/dev/shm/fungos-unibus-notices.6nHVu8/`; they are not durable artifacts.
The known virtual AF_VSOCK SSH-generator warning remains visible. These results
do not prove whole-system health, public interactive installation, personal SSH,
authenticated Unibus delivery, signed component update/rollback or Secure Boot.
They are local qualification evidence, not a v0.1 release announcement.

### Installed delivery, claim and reboot

A fresh private clone of QA8's UEFI installation passed the owner Unibus proof:
the empty default rejects registration, an authenticated producer reaches an
authorized consumer, incorrect tokens and ungranted capabilities are rejected,
the local owner readiness endpoint authenticates, and original configuration is
restored. The actual package-owned router ran under systemd's unprivileged
dynamic identity; neither executable was substituted by a test fixture.

The same clone passed Mycelium's inert-install, runtime first-contact sandbox,
package-safe enrollment, repair and native-update prewrite rejection checks.
A second disk-only boot verified unchanged peer, certificate, Iroh and SSH host
identity and repeated all Unibus checks. No external NIC, host port forwarding
or shared filesystem was attached. The runtime authority and short-lived claim
were generated privately inside the guest, not supplied in public media.

The first attempted clone exposed a **harness** systemd dependency error:
`Requires=unibus-router.service` cancelled its proof during an intentional
router restart, interrupting cleanup. It is retained as failed evidence, not a
pass. Helper fix `d19dc90` uses `Wants` plus `After`, has a regression test, and
was tested on a new clone; public ISO bytes did not change.

The successful marker log is `runtime-markers.log`, SHA-256
`0d12339c21fbf43659bf40e75d6781ee11ac8a7e2f23e739df742d2075f93e20`,
under the evidence root above. The private disk is
`/dev/shm/fungos-unibus-notices.6nHVu8/qa8-runtime-proof2/disk.raw`.
These results still do not establish personal SSH, signed component rollback,
public interactive installation, whole-system health or final v0.1 acceptance.

### Personal SSH and signed component recovery

Separate fresh clones of the successfully enrolled QA8 disk completed the
following checks; all owned QEMUs stopped and inspection loops detached.

- Personal certificate-only SSH succeeded for the granted identity. Enrollment
  alone granted no personal access; same-CA ungranted identity, wrong role and
  revoked certificate were rejected.
- Mycelium's real APT engine rejected an incorrect plan digest, missing admitted
  key, tampered signature and stale binding, then activated changed owner bytes.
  Failed-health rollback and interrupted activation recovery restored the prior
  healthy version and identity. Another disk-only boot verified persistence.
- Unibus used that same engine and its authenticated local owner readiness
  endpoint. Exact-digest rejection, successful lifecycle upgrade, failed-health
  rollback, conffile/identity preservation and reboot persistence passed. The
  healthy qualification version retains the exact owner executable; the failing
  version is explicitly an exit-only fault fixture, not a releasable application
  build. This is not changed-code or data-migration rollback evidence for Unibus.

| Marker log | SHA-256 |
|---|---|
| `access-markers.log` | `3cc25b8b0f4dc32cc05c1422433aec913e6cc0257fc3e9f1327ec71ddb855e51` |
| `apt2-markers.log` | `de9a749f8ed6cc2afdb40707c68d2744b13907af0385b028f662913e19de0951` |
| `unibus-apt-markers.log` | `8dc75a88c0a69dedf9b8636e11b176176f9c94777c8097db6624dcab3f024fa0` |

The new disposable, one-day test signer has public fingerprint
`95BEB9382917B4D6A71F338E6143616EFBDE00B5`; its private key never entered a
guest or public media. Mycelium generation is
`0a299035d6c0cf7e567d3bed076c4604d26b9d774df66a3d12e90ca6032d02cc`;
Unibus generation is
`be430e8b85441f1d736510b1040d07807964a8898867b196a62fe9ad41a96273`.
Signature and expiry checks stayed enabled, without a NIC or signature bypass.
The first APT attempt correctly refused a repository missing the installed QA6
rollback archive, before mutation. The passing generation includes that exact
archive. Neither failed attempt is counted as acceptance.

The failed runtime disk was losslessly converted to compressed qcow2: `qemu-img
check` passed and `qemu-img compare` reported identical images before the
temporary raw copy was removed. It remains private at
`/dev/shm/fungos-unibus-notices.6nHVu8/qa8-failed-runtime-retained/disk.qcow2`.
Reproducible extraction/build caches were removed; source archives and check
logs remain retained. Tmpfs disks are temporary, not an off-device backup.

### Public interactive install and first-boot findings

The unmodified QA8 ISO subsequently completed its real UEFI **Install** menu,
language/keyboard/account/timezone choices, guided partitioning and explicit
disk-write confirmation on a new 12 GiB virtual disk. The write confirmation
defaulted to **No**. Root was locked; the separately created local user could
authenticate and use sudo. No external preseed, injected kernel/initrd, shared
filesystem, physical disk or fleet credentials were used. A separate disk-only
invocation booted kernel `6.12.111+deb13-amd64` and reached login.

First-boot assessment found zero failed units and `is-system-running=running`.
SSH, networkd, resolved and the genuine unprivileged Unibus router were active.
Wired DHCP, a default route and an actual DNS query worked. Unibus retained its
loopback-only listener and empty routes/nodes; executable hashes matched the
owner packages. Mycelium and first-contact were correctly condition-skipped
without a claim. The known AF_VSOCK generator error remained in the journal;
no security scanner was run, so CVE/STIG clearance is **unknown**, not passed.

This public path exposed release-blocking divergence that the unattended path
did not cover:

- Debian's software picker selected GNOME by default; all optional tasks were
  manually deselected for this headless test.
- Before profile convergence, the selected rolling Debian mirror upgraded
  packages. For example `dhcpcd-base=1:10.1.0-11+deb13u4` was retained: a later
  snapshot `dist-upgrade` does not downgrade newer packages. This installation
  is not the locked installed inventory used by the earlier source closure.
- d-i's ifupdown DHCP configuration competed with networkd. Socket inspection
  observed both clients on the same interface.
- Linking resolved's stub before its service ran in the installer chroot broke
  DNS during d-i's subsequent APT finishing step, which logged fetch warnings.

Fix `71b2271` shares the public package policy with the private harness: no
rolling mirror, optional tasks or pre-profile upgrade. Fix `1ecdf1b` replaces
inherited physical-interface ifupdown configuration with loopback only and
retains working installer DNS behind the volatile stub path until reboot.
Syntax, public-preseed safety, resolver-transition and Linux overlay replacement
regressions passed. **These changes alter the ISO; QA8's runtime evidence does
not qualify the new bytes.** The public guest shut down cleanly; its private
disk/password remain outside release artifacts.

| Retained public evidence | SHA-256 |
|---|---|
| `public-install.log` | `4bbdc66dabbe70a269c6d5cfce2f29c1eb339a746780b265226d9c34c25bfb8e` |
| `public-health.log` | `43e52e90aa5544e9e288be46bd9ab1c7d4121c7c9070ef1fc6b0b740484a6499` |
| `public-disk-confirmation.png` | `05a70fd55e95d9206f0a4ffa6a251b897f84da3aab2788a4d31308f630060d1c` |
| `public-task-selection.png` | `f004b7ca7f0103c087056125df6409d05e6bf6b311df1c5e55fc7ba46e36ee85` |
| `public-install-complete.png` | `0e7f0d2472f53d9fbf17c1c5d37b1b4cdcf8741d39b48e1d69dffe28d5a59a3d` |

Still open for v0.1: fresh image qualification after these fixes, final source
and notice publication, release signing/recovery handoff and final
torrent/download promotion. Ordinary UEFI does not imply Secure Boot or
compliance certification.

## Preparatory public UEFI entry (2026-10-09)

The retained ISO from the earlier BIOS experiment has SHA-256
`c72c97966d464fa0299351121704ad753d7fa66a58f6dba8d5c1579b261e3c5d`.
It uses trixie snapshot `20261004T000000Z`, not the refreshed release baseline.
On Agora, QEMU with OVMF booted its real public UEFI menu. Selecting **Install**
reached the interactive language screen. No disk or network adapter was attached
for this menu-only check; it does not prove an interactive installation.

Raw QEMU framebuffer captures remain in
`/home/ajmwagar/.cache/fungos-uefi-qualification.4sfRDU/`:

| Capture | SHA-256 |
|---|---|
| `public-menu.ppm` | `995273a89248b1bcce5d805cc54d405a597fb3503e3b8757d5c2ee2322841619` |
| `public-interactive.ppm` | `28bf23c79e06344ad9910180c037d525f6427acdbca0b6930c3e3695212ab8bc` |

The public-menu QEMU was stopped afterward. The preexisting edge QEMU was not
changed. Final baseline/component enrollment and update gates remain open in
Marbles (`mycelium-v34j`, `mycelium-fungos-component-packaging`).

## Refreshed baseline install and reboot (2026-10-09)

Built the OS-only installer from committed source
`c1b1fc74bb8bd509abe16df758b819e565ca3878`, with explicit `SOURCE_COMMIT` for
the archived source tree. This candidate uses trixie snapshot
`20261009T000000Z`; it does **not** contain the pending Mycelium/Unibus owner
package bundle. It is not a v0.1.0 release candidate for operational use.

- ISO: `fungos-trixie-20261009-v2.iso`, SHA-256
  `ae1aabf257bc84880592c2cb213ff0c7c9051bfbf870d3944fd892eeb9fd0ffa`.
- Archived source SHA-256:
  `1281ca5672d357897bd70d7dd3b2c3958546ddec9d5d2ec10d87a14a951436e2`.
- Evidence root on Agora:
  `/home/ajmwagar/.cache/fungos-uefi-qualification.4sfRDU/`.

Both fresh disposable installations emitted `FUNGOS_INSTALL_COMPLETE` after
profile convergence and GRUB configuration. Separate disk-only BIOS and UEFI
boots reached `fungos login:` with kernel `6.12.111+deb13-amd64`. The UEFI log
records OVMF starting `\EFI\debian\shimx64.efi` from the installed GPT ESP.
BIOS harness exited zero. The copied pre-fix UEFI harness exited one because
GRUB's quiet default suppresses the kernel EFI banner; the corrected check in
`600235d` was independently applied to its retained logs and passed. This is
not a claim that the pre-fix harness exited successfully.

| Evidence | SHA-256 |
|---|---|
| `fresh-bios-proof/install.log` | `707f49b93327254107d8c1894a10acfa2db6c019471afb2423e47909e1b4ec5d` |
| `fresh-bios-proof/boot.log` | `3fd941d2648d566a50c6d9859942b2ae52457a3c16105c3bbba9e607d91a62a0` |
| `fresh-uefi-proof/install.log` | `508be188e6987454d0957bf6b33cf24e698e826880e88272154f279450112660` |
| `fresh-uefi-proof/boot.log` | `d38bd6a8577cfe00d9d93d064d02c41a5178c4fb21f335a4152137329770b50a` |
| `fresh-uefi-proof/reboot-persistence.log` | `c08f7e8b14ff9596f9dbc64752ddbf40e6c0c884c910c148be4fe9ecadcb575c` |

A further installed-disk-only UEFI boot reached login, then shut down using its
private QEMU monitor's ACPI power button. Read-only disk inspections before and
after that reboot found unchanged machine-ID and ED25519 host-public-key hashes,
and unchanged 214-row binary/source package inventory. Inventory SHA-256 is
`8c8ba0e00c2b50596b45d7c3b0f56feeca379ab75569f3f22ea69aae1986b40d`.
The shared SSH/networkd units are enabled; the generated host public key exists;
no first-contact claim environment or installer payload remains. Read-only loop
devices were detached and all qualification QEMUs stopped afterward.

The Debian `systemd-ssh-generator` AF_VSOCK CID warning remains visible. These
checks do not establish whole-system unit health, a completed public interactive
installation, production enrollment, personal certificate SSH, Secure Boot or
real component update/rollback. Those gates remain open.
