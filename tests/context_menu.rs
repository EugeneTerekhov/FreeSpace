//! US19: Explorer context-menu installation and removal in `HKCU`.
//!
//! The tests never touch the real Explorer menu: they register under a unique
//! `HKCU\Software\FreeSpaceTests\...` subtree and remove it afterwards.

mod common;

use std::path::{Path, PathBuf};

use freespace_platform::context_menu::{ContextMenu, MenuAction, MenuTarget};

fn test_menu() -> (ContextMenu, PathBuf) {
    let unique = format!(
        "Software\\FreeSpaceTests\\{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    );
    let executable = Path::new(r"C:\Program Files\FreeSpace Тест\freespace.exe").to_path_buf();

    (ContextMenu::user().with_base(unique), executable)
}

#[test]
fn installs_every_entry_with_a_quoted_command() {
    let (menu, executable) = test_menu();
    let entries = menu.install(&executable).expect("install context menu");

    assert_eq!(
        entries.len(),
        5,
        "two actions for files and folders, one for drives"
    );
    let installed = menu.installed_entries().expect("read entries");
    assert_eq!(installed.len(), entries.len());
    assert!(menu.is_installed().expect("installed"));

    let analyze = installed
        .iter()
        .find(|entry| entry.target == MenuTarget::Directory && entry.action == MenuAction::Analyze)
        .expect("folder analyze entry");
    assert!(analyze.command.starts_with('"'));
    assert!(analyze.command.contains("Program Files"));
    assert!(analyze.command.ends_with("%1"));
    assert!(
        analyze.command.contains("Тест"),
        "Unicode survives the registry"
    );

    let unlock = installed
        .iter()
        .find(|entry| entry.target == MenuTarget::File && entry.action == MenuAction::Unlock)
        .expect("file unlock entry");
    assert!(unlock.command.contains(" unlock "));

    menu.uninstall().expect("uninstall");
    assert!(!menu.is_installed().expect("uninstalled"));
    assert!(menu.installed_entries().expect("empty").is_empty());
}

#[test]
fn uninstall_is_idempotent_and_removes_subkeys() {
    let (menu, executable) = test_menu();
    menu.install(&executable).expect("install");

    menu.uninstall().expect("first removal");
    menu.uninstall().expect("second removal must succeed");

    assert!(menu.installed_entries().expect("empty").is_empty());
    assert!(!menu.is_installed().expect("not installed"));
}

#[test]
fn installed_command_survives_a_round_trip_through_the_registry() {
    let (menu, executable) = test_menu();
    let planned = menu.plan(&executable).expect("plan");

    menu.install(&executable).expect("install");
    let installed = menu.installed_entries().expect("read");
    menu.uninstall().expect("cleanup");

    for entry in planned {
        let stored = installed
            .iter()
            .find(|stored| stored.key == entry.key)
            .expect("every planned key is present");
        assert_eq!(stored.command, entry.command);
    }
}
