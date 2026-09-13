//! Scaffolding shared by the crate's unit tests.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// A uniquely named directory under the system temp directory that
/// deletes itself when dropped.
///
/// Tests that clean up on their last line leave their files behind
/// whenever an assertion fails — and a name derived from the process id
/// alone can then be inherited by a later run. Cleaning up in `Drop`
/// happens on the failing path too.
pub struct TempDir(PathBuf);

impl TempDir {
    /// Create `keyloom-<label>-<pid>-<n>/`, replacing any leftovers a
    /// killed run (which never drops anything) may have left at the
    /// same path.
    ///
    /// # Panics
    ///
    /// Panics when the directory cannot be created, which leaves the
    /// calling test unable to do anything meaningful anyway.
    pub fn new(label: &str) -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "keyloom-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a writable temp directory");
        Self(dir)
    }

    /// The directory itself, to build paths under.
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // Nothing useful can be done about a failure here: the test is
        // already over, and the next run replaces the directory anyway.
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
