//! The facade must not migrate secrets before it delegates static diagnostics.

#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use codewhale_secrets::{FileKeyringStore, KeyringStore};
use tempfile::TempDir;

#[test]
fn dispatcher_diagnostics_leave_legacy_secret_state_unchanged() {
    for (args, expected_tui_args, expects_json) in [
        (&["doctor"][..], &["doctor"][..], false),
        (&["doctor", "--json"][..], &["doctor", "--json"][..], false),
        (
            &["doctor", "--context-json"][..],
            &["doctor", "--context-json"][..],
            true,
        ),
        (
            &["setup", "--status"][..],
            &["setup", "--status"][..],
            false,
        ),
    ] {
        let fixture = TempDir::new().expect("fixture root");
        let sealed_home = fixture.path().join("sealed-home");
        let codewhale_home = fixture.path().join("sealed-codewhale-home");
        let primary_home = sealed_home.join(".codewhale");
        let legacy = sealed_home
            .join(".deepseek")
            .join("secrets")
            .join("secrets.json");
        let legacy_settings = sealed_home.join(".deepseek").join("settings.toml");
        let legacy_settings_bytes = b"default_mode = \"plan\"\n";
        FileKeyringStore::new(&legacy)
            .set("deepseek", "synthetic-legacy-fixture")
            .expect("seed synthetic legacy store");
        fs::write(&legacy_settings, legacy_settings_bytes).expect("seed legacy settings");
        let before_paths = relative_paths(&sealed_home);
        let before_legacy = fs::read(&legacy).expect("read synthetic legacy store");

        let receipt = fixture.path().join("delegated-args.txt");
        let fake_tui = fixture.path().join("fake-codewhale-tui");
        fs::write(
            &fake_tui,
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$DIAGNOSTIC_DISPATCH_RECEIPT\"\nif [ \"$1\" = doctor ] && [ \"$2\" = --context-json ]; then\n  printf '%s\\n' '{\"entries\":[]}'\nfi\n",
        )
        .expect("write fake TUI");
        let mut permissions = fs::metadata(&fake_tui)
            .expect("fake TUI metadata")
            .permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&fake_tui, permissions).expect("make fake TUI executable");

        let output = Command::new(codewhale_binary())
            .args(args)
            .env_clear()
            .env("HOME", &sealed_home)
            .env("USERPROFILE", &sealed_home)
            .env("CODEWHALE_HOME", &codewhale_home)
            .env("CODEWHALE_SECRET_BACKEND", "file")
            .env("DEEPSEEK_TUI_BIN", &fake_tui)
            .env("DIAGNOSTIC_DISPATCH_RECEIPT", &receipt)
            .output()
            .expect("run dispatcher diagnostic");

        assert!(
            output.status.success(),
            "dispatcher {args:?} failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            fs::read_to_string(&receipt)
                .expect("fake TUI receipt")
                .lines()
                .collect::<Vec<_>>(),
            expected_tui_args,
            "dispatcher must preserve the diagnostic command shape"
        );
        if expects_json {
            let report: serde_json::Value = serde_json::from_slice(&output.stdout)
                .unwrap_or_else(|error| {
                    panic!(
                        "facade {args:?} must preserve machine-readable output: {error}\nstdout:\n{}\nstderr:\n{}",
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    )
                });
            assert!(
                report["entries"].is_array(),
                "facade {args:?} must preserve the context source map\nstdout:\n{}",
                String::from_utf8_lossy(&output.stdout)
            );
        }
        assert_eq!(
            relative_paths(&sealed_home),
            before_paths,
            "dispatcher {args:?} must not create or migrate state below HOME"
        );
        assert_eq!(
            fs::read(&legacy).expect("read synthetic legacy store after diagnostic"),
            before_legacy,
            "dispatcher {args:?} must not rewrite the legacy store"
        );
        assert_eq!(
            fs::read(&legacy_settings).expect("read legacy settings after diagnostic"),
            legacy_settings_bytes,
            "dispatcher {args:?} must not rewrite legacy settings"
        );
        assert!(
            !primary_home.exists(),
            "dispatcher {args:?} must not create a primary Codewhale home or migrated state"
        );
        assert!(
            !codewhale_home.exists(),
            "dispatcher {args:?} must not create an explicit CODEWHALE_HOME"
        );
    }
}

fn relative_paths(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_relative_paths(root, root, &mut paths);
    paths.sort();
    paths
}

fn collect_relative_paths(root: &Path, current: &Path, paths: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(current).expect("read synthetic state directory");
    for entry in entries {
        let entry = entry.expect("synthetic state directory entry");
        let path = entry.path();
        paths.push(
            path.strip_prefix(root)
                .expect("synthetic path below root")
                .to_path_buf(),
        );
        if entry.file_type().expect("synthetic entry type").is_dir() {
            collect_relative_paths(root, &path, paths);
        }
    }
}

fn codewhale_binary() -> PathBuf {
    if let Some(path) = option_env!("CARGO_BIN_EXE_codewhale") {
        return PathBuf::from(path);
    }
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_codewhale") {
        return PathBuf::from(path);
    }

    let mut path = std::env::current_exe().expect("current test executable path");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.push(format!("codewhale{}", std::env::consts::EXE_SUFFIX));
    path
}
