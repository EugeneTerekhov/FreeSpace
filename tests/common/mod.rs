//! Shared filesystem fixtures for cross-crate integration tests.
//!
//! Every fixture lives in a temporary directory created by [`tempfile`]: tests
//! must never touch real user data. Administrative scenarios stay disabled
//! unless `FREESPACE_ADMIN_TESTS=1` is set explicitly.
#![allow(dead_code)]

use std::{
    fs,
    io::Write as _,
    path::{Path, PathBuf},
};

use freespace_platform::paths;
use tempfile::TempDir;

/// A self-cleaning temporary directory with helpers for building fixtures.
#[derive(Debug)]
pub struct TestTree {
    root: TempDir,
}

impl TestTree {
    /// Creates an empty fixture.
    ///
    /// # Panics
    /// Panics when the temporary directory cannot be created.
    #[must_use]
    pub fn new() -> Self {
        Self {
            root: tempfile::tempdir().expect("create temporary fixture directory"),
        }
    }

    /// Creates the standard fixture used by scanning and analysis tests.
    #[must_use]
    pub fn sample() -> Self {
        let fixture = Self::new();
        fixture.write("alpha/first.txt", &[b'a'; 10]);
        fixture.write("alpha/second.txt", &[b'b'; 20]);
        fixture.write("beta/nested/gamma.dat", &[b'c'; 100]);
        fixture.mkdir("empty");
        fixture.write("UNICODE-имя/файл.txt", b"hello");
        fixture.write("link-target.txt", b"target");

        fixture
    }

    /// Fixture root.
    #[must_use]
    pub fn root(&self) -> &Path {
        self.root.path()
    }

    /// Absolute path of a fixture entry.
    #[must_use]
    pub fn path(&self, relative: impl AsRef<Path>) -> PathBuf {
        self.root.path().join(relative)
    }

    /// Creates a directory (and parents) inside the fixture.
    pub fn mkdir(&self, relative: impl AsRef<Path>) -> PathBuf {
        let path = self.path(relative);
        fs::create_dir_all(&path).expect("create fixture directory");

        path
    }

    /// Writes a file (creating parents) and returns its absolute path.
    pub fn write(&self, relative: impl AsRef<Path>, bytes: &[u8]) -> PathBuf {
        let path = self.path(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create fixture parent directory");
        }
        let mut file = fs::File::create(&path).expect("create fixture file");
        file.write_all(bytes).expect("write fixture file");

        path
    }

    /// Writes a deterministic file of `size` bytes; the content is derived from
    /// the path so duplicate-detection tests stay reproducible.
    pub fn write_filled(&self, relative: impl AsRef<Path>, size: usize) -> PathBuf {
        let relative = relative.as_ref();
        let seed = relative.to_string_lossy().bytes().sum::<u8>();
        let bytes = (0..size)
            .map(|index| seed.wrapping_add((index % 251) as u8))
            .collect::<Vec<_>>();

        self.write(relative, &bytes)
    }

    /// Builds a nested directory whose absolute path exceeds 260 characters and
    /// returns the deepest directory.
    ///
    /// The directories are created through verbatim paths, so the fixture also
    /// proves that the platform layer does not depend on `LongPathsEnabled`.
    #[must_use]
    pub fn long_directory(&self, minimum_chars: usize) -> PathBuf {
        let mut current = self.root.path().to_path_buf();
        let mut index = 0usize;

        while current.as_os_str().len() < minimum_chars.max(270) {
            let component = format!("segment-{index:04}-{}", "x".repeat(40));
            current.push(component);
            let verbatim = paths::to_verbatim(&current).expect("verbatim directory path");
            fs::create_dir_all(&verbatim).expect("create long fixture directory");
            index += 1;
        }

        current
    }

    /// Total logical size of every regular file in the fixture.
    ///
    /// # Panics
    /// Panics when the fixture cannot be walked.
    #[must_use]
    pub fn total_file_bytes(&self) -> u64 {
        let mut total = 0;
        let mut stack = vec![self.root.path().to_path_buf()];

        while let Some(directory) = stack.pop() {
            for entry in fs::read_dir(&directory).expect("read fixture directory") {
                let entry = entry.expect("read fixture entry");
                let metadata = entry.metadata().expect("read fixture metadata");
                if metadata.is_dir() {
                    stack.push(entry.path());
                } else {
                    total += metadata.len();
                }
            }
        }

        total
    }
}

impl Default for TestTree {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns `true` when administrative tests were explicitly enabled.
#[must_use]
pub fn admin_tests_enabled() -> bool {
    std::env::var("FREESPACE_ADMIN_TESTS").is_ok_and(|value| value == "1")
}

/// Returns an [`Option`] guard for administrative tests.
///
/// Usage:
/// ```ignore
/// let Some(_guard) = require_admin() else { return };
/// ```
#[must_use]
pub fn require_admin() -> Option<&'static str> {
    admin_tests_enabled().then_some("administrative tests enabled")
}

/// Returns a stable list of fixture file sizes used to verify scan totals.
#[must_use]
pub fn sample_sizes() -> [u64; 5] {
    [10, 20, 100, 5, 6]
}
