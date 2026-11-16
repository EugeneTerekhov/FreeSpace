//! US23: regression test for the offline-only contract (FR-094, FR-095).
//!
//! The application must never open a network connection or send telemetry. This
//! test inspects the workspace sources instead of trusting a runtime check, so a
//! future dependency or URL cannot slip in unnoticed.

mod common;

use std::path::{Path, PathBuf};

/// Crates that would introduce network access.
const FORBIDDEN_CRATES: [&str; 12] = [
    "reqwest",
    "hyper",
    "ureq",
    "surf",
    "isahc",
    "curl",
    "tokio-tungstenite",
    "tungstenite",
    "sentry",
    "opentelemetry",
    "segment",
    "mixpanel",
];

/// Source markers that indicate network or telemetry code.
const FORBIDDEN_MARKERS: [&str; 8] = [
    "TcpStream",
    "UdpSocket",
    "std::net::",
    "sentry::",
    "opentelemetry",
    "telemetry",
    "analytics",
    "track_event",
];

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn rust_sources(root: &Path, output: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.file_name().is_some_and(|name| name == "target") {
            continue;
        }
        if path.is_dir() {
            rust_sources(&path, output);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            output.push(path);
        }
    }
}

#[test]
fn no_manifest_pulls_in_a_network_stack() {
    let root = workspace();
    let mut manifests = Vec::new();
    collect_manifests(&root, &mut manifests);
    assert!(!manifests.is_empty(), "workspace manifests were not found");

    for manifest in manifests {
        let text = std::fs::read_to_string(&manifest).expect("read manifest");
        let lower = text.to_lowercase();
        for forbidden in FORBIDDEN_CRATES {
            assert!(
                !lower.contains(forbidden),
                "{} references the network crate `{forbidden}`",
                manifest.display()
            );
        }
    }
}

#[test]
fn production_sources_do_not_open_sockets_or_telemetry() {
    let root = workspace();
    let mut sources = Vec::new();
    for directory in ["crates", "apps"] {
        rust_sources(&root.join(directory), &mut sources);
    }
    assert!(!sources.is_empty(), "no Rust sources were found");

    for source in sources {
        let text = std::fs::read_to_string(&source).expect("read source");
        for marker in FORBIDDEN_MARKERS {
            assert!(
                !text.contains(marker),
                "{} contains the forbidden marker `{marker}`",
                source.display()
            );
        }
    }
}

#[test]
fn production_sources_never_resolve_system_profile_directories() {
    let root = workspace();
    let mut sources = Vec::new();
    for directory in ["crates", "apps"] {
        rust_sources(&root.join(directory), &mut sources);
    }

    let forbidden = [
        "var_os(\"APPDATA\")",
        "var(\"APPDATA\")",
        "var_os(\"LOCALAPPDATA\")",
        "var(\"LOCALAPPDATA\")",
        "var_os(\"ProgramData\")",
        "var(\"ProgramData\")",
        "env::temp_dir",
        "std::env::temp_dir",
    ];

    for source in sources {
        let text = std::fs::read_to_string(&source).expect("read source");
        let production = text.split("#[cfg(test)]").next().unwrap_or(text.as_str());
        for marker in forbidden {
            assert!(
                !production.contains(marker),
                "{} resolves the system directory via `{marker}`",
                source.display()
            );
        }
    }
}

#[test]
fn exports_and_documentation_contain_no_upload_endpoints() {
    let root = workspace();
    let mut sources = Vec::new();
    for directory in ["crates", "apps"] {
        rust_sources(&root.join(directory), &mut sources);
    }

    for source in sources {
        let text = std::fs::read_to_string(&source).expect("read source");
        let production = text.split("#[cfg(test)]").next().unwrap_or(text.as_str());
        for (line_number, line) in production.lines().enumerate() {
            let lower = line.to_lowercase();
            if lower.contains("http://") || lower.contains("https://") {
                // Documentation links inside comments and XML namespace
                // identifiers are tolerated; executable requests are not.
                let namespace =
                    lower.contains("schemas.microsoft.com") || lower.contains("www.w3.org");
                assert!(
                    namespace || line.trim_start().starts_with("//"),
                    "{}:{} contains a URL outside a comment",
                    source.display(),
                    line_number + 1
                );
            }
        }
    }
}

fn collect_manifests(directory: &Path, output: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .file_name()
            .is_some_and(|name| name == "target" || name == ".git")
        {
            continue;
        }
        if path.is_dir() {
            collect_manifests(&path, output);
        } else if path.file_name().is_some_and(|name| name == "Cargo.toml") {
            output.push(path);
        }
    }
}
