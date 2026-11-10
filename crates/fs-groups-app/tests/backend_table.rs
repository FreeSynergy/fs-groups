//! The Admin backend against the table of commands: the catalog, the refusal of what is not there yet, the wiring.

mod common;

use common::World;
use fs_admin_frontend::{Backend, BackendError, Values};
use fs_groups_app::{RunError, plan_change};
use fs_groups_core::describe::{COMMANDS, GROUP, unsupported_commands};
use fs_types::{BasicType, Edit};

#[test]
fn the_catalog_holds_every_type_with_every_command() {
    let w = World::new();
    let catalog = w.remote.catalog().unwrap();
    assert_eq!(catalog.len(), fs_groups_core::describe::TYPES.len());
    let commands: usize = catalog.iter().map(|t| t.commands.len()).sum();
    assert_eq!(commands, COMMANDS.len());
    let locked: Vec<String> = catalog
        .iter()
        .flat_map(|t| {
            t.commands
                .iter()
                .filter(|c| !c.supported)
                .map(move |c| format!("{}.{}", t.name, c.name))
        })
        .collect();
    let mut expected = unsupported_commands();
    expected.sort();
    let mut locked = locked;
    locked.sort();
    assert_eq!(locked, expected);
}

#[test]
fn unsupported_commands_are_refused_before_any_call() {
    let w = World::new();
    for name in unsupported_commands() {
        let (t, c) = name.split_once('.').unwrap();
        let t = fs_groups_core::describe::TYPES
            .iter()
            .find(|x| **x == t)
            .unwrap();
        let backend = w.of(t);
        assert!(!backend.supported(c), "{name}");
        assert_eq!(
            backend.run_typed(c, Some("x"), &Values::new()),
            Err(RunError::Unsupported(name.clone())),
            "typed refusal"
        );
        assert_eq!(
            backend.run(c, Some("x"), &Values::new()),
            Err(BackendError::NotFound)
        );
    }
    // and the ones the table lets through are announced as supported
    assert!(w.of(GROUP).supported("create") && w.of(GROUP).supported("change"));
    assert!(!w.of(GROUP).supported("nonsense"));
    assert_eq!(
        w.of(GROUP).run_typed("nonsense", None, &Values::new()),
        Err(RunError::NotFound)
    );
}

#[test]
fn every_supported_command_of_the_table_is_wired() {
    let w = World::new();
    for c in COMMANDS
        .iter()
        .filter(|c| c.supported && c.name != "change")
    {
        let t = fs_groups_core::describe::TYPES
            .iter()
            .find(|t| **t == c.data_type)
            .unwrap();
        // An empty input is refused as a bad value (or listed), never as "in the table but not wired".
        let answer = w.of(t).run_typed(c.name, None, &Values::new());
        if let Err(RunError::Unavailable(text)) = &answer {
            assert!(
                !text.contains("not wired"),
                "{}.{}: {text}",
                c.data_type,
                c.name
            );
        }
    }
}

#[test]
fn a_change_is_mapped_field_by_field() {
    let plan = plan_change(&Edit::new().set(&["name"], BasicType::Text, &["A"]).set(
        &["kind"],
        BasicType::Text,
        &[],
    ))
    .unwrap();
    assert_eq!(plan.name.as_deref(), Some("A"));
    assert_eq!(plan.kind.as_deref(), Some(""), "cleared");
    assert!(plan.flow.is_none(), "not in the edit: unchanged");
    assert!(matches!(
        plan_change(&Edit::new().set(&["parents"], BasicType::Reference, &["x"])),
        Err(RunError::Bad { .. })
    ));
    assert!(matches!(
        plan_change(&Edit::new().set(&["rightFlow"], BasicType::Choice, &["sideways"])),
        Err(RunError::Bad { .. })
    ));
}
