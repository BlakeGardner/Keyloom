//! Keyloom's own copy of xremap.
//!
//! When no xremap is installed, first-run setup can download the
//! release Keyloom was tested with from the xremap project's GitHub
//! releases and keep it in the user's `~/.local/bin`, the directory the
//! XDG base directory specification sets aside for a user's own
//! executables. No administrator access is involved. The download is
//! checked against a digest recorded here before anything is written,
//! then unpacked, made executable, asked for its version, and only then
//! moved into place. Digests of the unpacked binaries are recorded too:
//! they are how a binary at that path is recognized as Keyloom's own
//! rather than one the user put there, which Keyloom never replaces.

use std::fmt::{self, Write as _};
use std::fs;
use std::io::{Cursor, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::xremap;

/// The xremap release Keyloom downloads: its `full` build, which has a
/// client for every desktop xremap supports and understands
/// `--desktop`. Bump together with `XREMAP_VERSION` in the CI workflow
/// and the layer harness script (a test checks that they agree) and add
/// the new release's digests to [`RELEASES`].
pub const RELEASE: &str = "0.15.13";

/// The processor name xremap's release assets use for this build of
/// Keyloom; `None` where xremap publishes no binary.
#[cfg(target_arch = "x86_64")]
pub const ARCH: Option<&str> = Some("x86_64");
#[cfg(target_arch = "aarch64")]
pub const ARCH: Option<&str> = Some("aarch64");
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
pub const ARCH: Option<&str> = None;

/// One published build of a release: the SHA-256 of the zip GitHub
/// serves, checked before it is unpacked, and of the `xremap` binary
/// inside it, by which an installed copy is recognized as Keyloom's.
struct Build {
    arch: &'static str,
    zip: &'static str,
    binary: &'static str,
}

struct Release {
    version: &'static str,
    builds: [Build; 2],
}

/// Every release Keyloom has ever downloaded, newest first, so a copy
/// from an earlier Keyloom is still recognized as Keyloom's and offered
/// an update.
const RELEASES: &[Release] = &[Release {
    version: "0.15.13",
    builds: [
        Build {
            arch: "x86_64",
            zip: "4b82bdc279f9c4d96292a4ecc31107905eacfd190398946f96360c1ef157c13e",
            binary: "57acf06438cfe7d153114a892dc81ccf33f6b4130c7a6c70e344d0df2944abbc",
        },
        Build {
            arch: "aarch64",
            zip: "c8dd332046a43f643c589c2b7591c314e44157ea1113ee5640bd346a8385f8fc",
            binary: "4fe15d6faf77b3c1fded52e0162f304915b1c98e9ea1d9feb375407836e78559",
        },
    ],
}];

/// The one entry in a release zip.
const ENTRY: &str = "xremap";

/// The most a release zip may be; the real ones are around 3 MB.
const MAX_ZIP_BYTES: u64 = 32 * 1024 * 1024;

/// The most the unpacked binary may be; the real ones are around 8 MB.
const MAX_BINARY_BYTES: u64 = 64 * 1024 * 1024;

/// The release zip Keyloom downloads for this processor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asset {
    pub version: &'static str,
    pub arch: &'static str,
    pub url: String,
    /// SHA-256 of the zip, in lowercase hex.
    pub sha256: &'static str,
}

/// Where GitHub serves the `full` build of a release for a processor.
pub fn asset_url(version: &str, arch: &str) -> String {
    format!(
        "https://github.com/xremap/xremap/releases/download/v{version}/xremap-linux-{arch}-full.zip"
    )
}

/// The zip to download for this processor: [`RELEASE`] for [`ARCH`],
/// or `None` where xremap publishes no binary.
pub fn asset() -> Option<Asset> {
    let arch = ARCH?;
    let release = RELEASES.first()?;
    let build = release.builds.iter().find(|build| build.arch == arch)?;
    Some(Asset {
        version: release.version,
        arch,
        url: asset_url(release.version, arch),
        sha256: build.zip,
    })
}

/// The user's own executables directory, `~/.local/bin`, from `HOME`.
pub fn bin_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
        .filter(|home| home.is_absolute())
        .map(|home| home.join(".local").join("bin"))
}

/// Where Keyloom keeps its own xremap: `~/.local/bin/xremap`.
pub fn managed_path() -> Option<PathBuf> {
    Some(bin_dir()?.join(ENTRY))
}

/// The SHA-256 of some bytes, in lowercase hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        // Writing into a String cannot fail.
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// The release a binary belongs to, when it is byte for byte one Keyloom
/// ships; `None` for anything else, whoever built it.
pub fn known_release(binary: &[u8]) -> Option<&'static str> {
    let digest = sha256_hex(binary);
    RELEASES
        .iter()
        .find(|release| release.builds.iter().any(|build| build.binary == digest))
        .map(|release| release.version)
}

/// Why xremap could not be downloaded and installed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// xremap publishes no binary for this processor.
    NoAsset { arch: &'static str },
    /// No home directory to put it in.
    NoHome,
    /// The download did not complete; the text is the transport's.
    Network(String),
    /// The download is not the file Keyloom expected, so nothing of it
    /// was kept.
    Digest {
        expected: &'static str,
        actual: String,
    },
    /// The zip could not be read, or holds no `xremap`.
    Archive(String),
    /// The binary could not be written or moved into place.
    Io(String),
    /// The unpacked binary did not answer `--version`.
    WontRun { dir: PathBuf, detail: String },
}

impl fmt::Display for Error {
    /// Written to compose after a prefix, as error messages do:
    /// lowercase, no trailing punctuation.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoAsset { arch } => {
                write!(f, "there is no xremap release for this processor ({arch})")
            }
            Self::NoHome => f.write_str("there is no home directory to download xremap into"),
            Self::Network(text) => write!(f, "the download did not complete: {text}"),
            Self::Digest { expected, actual } => write!(
                f,
                "the download is not the file Keyloom expected (SHA-256 {actual} instead of \
                 {expected}), so it was discarded"
            ),
            Self::Archive(text) => {
                write!(f, "the downloaded archive could not be unpacked: {text}")
            }
            Self::Io(text) => write!(f, "the binary could not be put in place: {text}"),
            Self::WontRun { dir, detail } => write!(
                f,
                "the downloaded xremap does not run from {} ({detail}); a home directory \
                 mounted without permission to run programs (noexec) would cause this",
                dir.display()
            ),
        }
    }
}

impl std::error::Error for Error {}

/// Download the release for this processor into [`managed_path`],
/// replacing whatever is there once the new binary has proven to run.
/// Only the project's GitHub release is contacted, over HTTPS.
///
/// # Errors
///
/// When there is no release for this processor or no home directory,
/// when the download fails or is not the expected file, or when the
/// binary cannot be unpacked, written, or run.
pub async fn download_and_install() -> Result<PathBuf, Error> {
    let asset = asset().ok_or(Error::NoAsset {
        arch: std::env::consts::ARCH,
    })?;
    let dest = managed_path().ok_or(Error::NoHome)?;
    let installed = dest.clone();
    // Downloading, hashing, and file work all block; none of it belongs
    // on the interface's thread.
    tokio::task::spawn_blocking(move || {
        let zip = fetch(&asset.url)?;
        verify(&zip, asset.sha256)?;
        let binary = unpack(&zip)?;
        install_binary(&binary, &dest)
    })
    .await
    .map_err(|err| Error::Io(format!("the download stopped early: {err}")))??;
    Ok(installed)
}

/// Fetch a release zip into memory: bounded in size and time, since
/// there is no way to cancel it from the interface.
fn fetch(url: &str) -> Result<Vec<u8>, Error> {
    let network = |err: ureq::Error| Error::Network(err.to_string());
    let config = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_global(Some(Duration::from_secs(300)))
        .user_agent(concat!("Keyloom/", env!("CARGO_PKG_VERSION")))
        .build();
    let agent = ureq::Agent::new_with_config(config);
    let mut response = agent.get(url).call().map_err(network)?;
    response
        .body_mut()
        .with_config()
        .limit(MAX_ZIP_BYTES)
        .read_to_vec()
        .map_err(network)
}

/// Check a download against the digest recorded for it.
///
/// # Errors
///
/// When the digests differ.
pub(crate) fn verify(bytes: &[u8], sha256: &'static str) -> Result<(), Error> {
    let actual = sha256_hex(bytes);
    if actual.eq_ignore_ascii_case(sha256) {
        Ok(())
    } else {
        Err(Error::Digest {
            expected: sha256,
            actual,
        })
    }
}

/// The `xremap` binary inside a release zip.
///
/// # Errors
///
/// When the bytes are not a zip, hold no file named `xremap`, or that
/// file is implausibly large or cut short.
pub(crate) fn unpack(zip: &[u8]) -> Result<Vec<u8>, Error> {
    let archive = |err: &dyn fmt::Display| Error::Archive(err.to_string());
    let mut zip = zip::ZipArchive::new(Cursor::new(zip)).map_err(|err| archive(&err))?;
    let mut entry = zip
        .by_name(ENTRY)
        .map_err(|err| Error::Archive(format!("no file named {ENTRY}: {err}")))?;
    if !entry.is_file() {
        return Err(Error::Archive(format!("{ENTRY} is not a file")));
    }
    let size = entry.size();
    if size > MAX_BINARY_BYTES {
        return Err(Error::Archive(format!("{ENTRY} claims to be {size} bytes")));
    }
    let mut binary = Vec::with_capacity(usize::try_from(size).unwrap_or_default());
    (&mut entry)
        .take(MAX_BINARY_BYTES)
        .read_to_end(&mut binary)
        .map_err(|err| archive(&err))?;
    if u64::try_from(binary.len()).ok() != Some(size) {
        return Err(Error::Archive(format!("{ENTRY} is cut short")));
    }
    Ok(binary)
}

/// Whether a binary runs here, judged by its answer to `--version`.
///
/// # Errors
///
/// With what went wrong: the program could not be started (a home
/// mounted `noexec`, a binary for another processor) or answered
/// unexpectedly.
pub(crate) fn check_runs(path: &Path) -> Result<String, String> {
    let output = Command::new(path)
        .arg("--version")
        .output()
        .map_err(|err| err.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let banner = stdout.trim();
    if output.status.success() && banner.starts_with(ENTRY) {
        return Ok(banner.to_owned());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let said = if banner.is_empty() {
        stderr.trim()
    } else {
        banner
    };
    Err(if said.is_empty() {
        format!("--version {}", output.status)
    } else {
        format!("--version {} saying {said:?}", output.status)
    })
}

/// Put a binary in place as an executable: written beside its
/// destination under a name of its own, proven to run, and only then
/// renamed over whatever is there. A binary that is not one Keyloom
/// ships is kept beside it first, like a hand-written configuration.
///
/// # Errors
///
/// When the directory or file cannot be written, or the binary does
/// not run; nothing is left behind then.
pub(crate) fn install_binary(binary: &[u8], dest: &Path) -> Result<(), Error> {
    let dir = dest
        .parent()
        .ok_or_else(|| Error::Io("the destination has no directory".to_owned()))?;
    fs::create_dir_all(dir).map_err(|err| Error::Io(err.to_string()))?;
    let tmp = temp_path(dest);
    let result = write_and_check(binary, &tmp, dest);
    if result.is_err() {
        // Nothing half-installed may be left where the next check would
        // take it for an xremap.
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn write_and_check(binary: &[u8], tmp: &Path, dest: &Path) -> Result<(), Error> {
    let io_failed = |err: std::io::Error| Error::Io(err.to_string());
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o755)
        .open(tmp)
        .map_err(io_failed)?;
    file.write_all(binary).map_err(io_failed)?;
    file.sync_all().map_err(io_failed)?;
    drop(file);
    check_runs(tmp).map_err(|detail| Error::WontRun {
        dir: dest.parent().map(Path::to_path_buf).unwrap_or_default(),
        detail,
    })?;
    if let Ok(existing) = fs::read(dest)
        && known_release(&existing).is_none()
    {
        xremap::backup(dest).map_err(io_failed)?;
    }
    fs::rename(tmp, dest).map_err(io_failed)
}

/// A name beside `dest` that no other download of this process uses,
/// so two downloads started from setup cannot trip over each other.
fn temp_path(dest: &Path) -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let name = dest.file_name().map_or_else(
        || ENTRY.to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    dest.with_file_name(format!(
        ".{name}.{}.{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use zip::write::SimpleFileOptions;

    use super::*;
    use crate::testing::TempDir;

    /// A zip holding the given files, the way xremap's releases are made.
    fn archive(entries: &[(&str, &[u8])], method: zip::CompressionMethod) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, data) in entries {
            writer
                .start_file(
                    *name,
                    SimpleFileOptions::default()
                        .compression_method(method)
                        .unix_permissions(0o755),
                )
                .unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    /// A stand-in binary: a script answering `--version` like xremap.
    const FAKE_XREMAP: &[u8] = b"#!/bin/sh\necho xremap 0.15.13\n";

    #[test]
    fn the_asset_is_the_full_build_of_the_pinned_release_for_this_processor() {
        assert_eq!(
            asset_url("0.15.13", "x86_64"),
            "https://github.com/xremap/xremap/releases/download/v0.15.13/xremap-linux-x86_64-full.zip"
        );
        assert_eq!(
            RELEASES[0].version, RELEASE,
            "the newest release is the one shipped"
        );
        for release in RELEASES {
            for build in &release.builds {
                for digest in [build.zip, build.binary] {
                    assert_eq!(digest.len(), 64, "{}: a SHA-256 in hex", release.version);
                    assert!(
                        digest
                            .bytes()
                            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
                    );
                }
                assert_ne!(build.zip, build.binary);
            }
        }
        match (ARCH, asset()) {
            (Some(arch), Some(asset)) => {
                assert_eq!(asset.version, RELEASE);
                assert_eq!(asset.arch, arch);
                assert_eq!(asset.url, asset_url(RELEASE, arch));
                assert!(
                    RELEASES[0]
                        .builds
                        .iter()
                        .any(|build| build.zip == asset.sha256)
                );
            }
            (None, None) => {}
            (arch, asset) => panic!("an asset exactly when there is a release: {arch:?} {asset:?}"),
        }
    }

    #[test]
    fn downloads_are_checked_against_the_recorded_digest() {
        const HELLO: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
        assert_eq!(sha256_hex(b"hello"), HELLO);
        assert_eq!(verify(b"hello", HELLO), Ok(()));
        assert_eq!(
            verify(
                b"hello",
                "2CF24DBA5FB0A30E26E83B2AC5B9E29E1B161E5C1FA7425E73043362938B9824"
            ),
            Ok(()),
            "case does not matter"
        );
        assert_eq!(
            verify(b"hellp", HELLO),
            Err(Error::Digest {
                expected: HELLO,
                actual: sha256_hex(b"hellp"),
            })
        );
        assert_eq!(known_release(FAKE_XREMAP), None);
        assert_eq!(known_release(b""), None);
    }

    #[test]
    fn the_xremap_entry_is_read_out_of_a_release_zip() {
        let payload = b"ELF\0not really".as_slice();
        let deflated = archive(&[("xremap", payload)], zip::CompressionMethod::Deflated);
        assert_eq!(unpack(&deflated).unwrap(), payload);
        let stored = archive(&[("xremap", payload)], zip::CompressionMethod::Stored);
        assert_eq!(unpack(&stored).unwrap(), payload);
        let with_extras = archive(
            &[("README.md", b"hi".as_slice()), ("xremap", payload)],
            zip::CompressionMethod::Deflated,
        );
        assert_eq!(
            unpack(&with_extras).unwrap(),
            payload,
            "other files are ignored"
        );

        let nested = archive(&[("dir/xremap", payload)], zip::CompressionMethod::Deflated);
        assert!(
            matches!(unpack(&nested), Err(Error::Archive(_))),
            "root entry only"
        );
        let empty = archive(&[], zip::CompressionMethod::Deflated);
        assert!(matches!(unpack(&empty), Err(Error::Archive(_))));
        assert!(matches!(
            unpack(b"not a zip at all"),
            Err(Error::Archive(_))
        ));
    }

    #[test]
    fn the_binary_is_proven_to_run_before_it_replaces_anything() {
        let dir = TempDir::new("install-binary");
        let bin = dir.path().join(".local").join("bin");
        let dest = bin.join("xremap");
        let leftovers = || {
            fs::read_dir(&bin)
                .unwrap()
                .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                .filter(|name| name.ends_with(".tmp"))
                .count()
        };

        assert_eq!(
            install_binary(FAKE_XREMAP, &dest),
            Ok(()),
            "the directory is created"
        );
        assert_eq!(fs::read(&dest).unwrap(), FAKE_XREMAP);
        assert_ne!(fs::metadata(&dest).unwrap().permissions().mode() & 0o111, 0);
        assert_eq!(check_runs(&dest).unwrap(), "xremap 0.15.13");
        assert_eq!(leftovers(), 0);

        // A binary that answers wrongly, or not at all, changes nothing.
        let wrong = b"#!/bin/sh\necho something else\n";
        assert!(matches!(
            install_binary(wrong, &dest),
            Err(Error::WontRun { .. })
        ));
        assert!(matches!(
            install_binary(b"not a program", &dest),
            Err(Error::WontRun { .. })
        ));
        assert_eq!(fs::read(&dest).unwrap(), FAKE_XREMAP);
        assert_eq!(leftovers(), 0);

        // Replacing a binary that is not one Keyloom ships keeps it beside.
        let newer = b"#!/bin/sh\necho xremap 0.15.14\n";
        assert_eq!(install_binary(newer, &dest), Ok(()));
        assert_eq!(fs::read(&dest).unwrap(), newer);
        assert_eq!(fs::read(bin.join("xremap.bak")).unwrap(), FAKE_XREMAP);
        assert_eq!(leftovers(), 0);

        let missing = dir.path().join("nowhere").join("xremap");
        assert!(check_runs(&missing).is_err());
    }

    #[test]
    fn errors_read_as_reasons() {
        let errors = [
            Error::NoAsset { arch: "riscv64" },
            Error::NoHome,
            Error::Network("timed out".to_owned()),
            Error::Digest {
                expected: "aa",
                actual: "bb".to_owned(),
            },
            Error::Archive("bad".to_owned()),
            Error::Io("bad".to_owned()),
            Error::WontRun {
                dir: PathBuf::from("/home/me/.local/bin"),
                detail: "Exec format error".to_owned(),
            },
        ];
        for error in errors {
            let text = error.to_string();
            assert!(text.starts_with(|c: char| c.is_ascii_lowercase()), "{text}");
            assert!(!text.ends_with('.'), "{text}");
        }
    }

    #[test]
    fn the_pinned_release_matches_ci_and_the_layer_harness() {
        let ci = include_str!("../.github/workflows/ci.yml");
        assert!(
            ci.contains(&format!("XREMAP_VERSION: {RELEASE}\n")),
            "CI validates generated configs against the release Keyloom installs"
        );
        let harness = include_str!("../scripts/verify-layers-with-xremap.sh");
        assert!(
            harness.contains(&format!("${{XREMAP_VERSION:-{RELEASE}}}")),
            "the harness builds the release Keyloom installs"
        );
    }

    /// Downloads the real release: CI runs it explicitly, so that the
    /// digests recorded here are known to be GitHub's.
    #[test]
    #[ignore = "downloads ~3 MB from GitHub and runs the unpacked binary's --version"]
    fn the_pinned_asset_is_what_github_serves() {
        let asset = asset().expect("a release for this processor");
        let zip = fetch(&asset.url).unwrap();
        verify(&zip, asset.sha256).unwrap();
        let binary = unpack(&zip).unwrap();
        assert_eq!(known_release(&binary), Some(RELEASE));
        let dir = TempDir::new("install-release");
        let dest = dir.path().join("bin").join("xremap");
        install_binary(&binary, &dest).unwrap();
        assert_eq!(check_runs(&dest).unwrap(), format!("xremap {RELEASE}"));
    }
}
