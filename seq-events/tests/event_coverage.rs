use std::collections::HashSet;

use serde::Deserialize;
use syn::Item;

#[derive(Deserialize)]
struct Manifest {
    schema_version: u32,
    contract: String,
    source: String,
    events: Vec<ManifestEvent>,
}

#[derive(Deserialize)]
struct ManifestEvent {
    name: String,
    family: String,
    internal_only: bool,
    internal_only_reason: Option<String>,
}

#[test]
fn manifest_covers_every_event_variant_exactly_once() {
    let source = include_str!("../src/lib.rs");
    let syntax = syn::parse_file(source).expect("seq-events source parses");
    let event_enum = syntax
        .items
        .iter()
        .find_map(|item| match item {
            Item::Enum(item) if item.ident == "Event" => Some(item),
            _ => None,
        })
        .expect("Event enum exists");
    let contract_names: Vec<_> = event_enum
        .variants
        .iter()
        .map(|variant| variant.ident.to_string())
        .collect();

    let manifest: Manifest = serde_json::from_str(include_str!("../event-coverage.json"))
        .expect("event coverage manifest parses");
    assert_eq!(manifest.schema_version, 1);
    assert_eq!(manifest.contract, "seq_events::Event");
    assert_eq!(manifest.source, "seq-events/src/lib.rs");

    let manifest_names: Vec<_> = manifest
        .events
        .iter()
        .map(|event| event.name.clone())
        .collect();
    assert_eq!(manifest_names, contract_names, "manifest is stale");
    assert_eq!(
        manifest_names.iter().collect::<HashSet<_>>().len(),
        manifest_names.len(),
        "manifest contains a duplicate event"
    );

    for event in &manifest.events {
        assert!(!event.family.is_empty(), "{} has no family", event.name);
        assert_eq!(
            event.internal_only,
            event
                .internal_only_reason
                .as_ref()
                .is_some_and(|reason| !reason.trim().is_empty()),
            "{} has inconsistent internal-only documentation",
            event.name
        );
    }
}
