use std::collections::BTreeSet;

use serde::Deserialize;
use syn::visit::{self, Visit};
use syn::{Expr, ExprMatch, Item, Pat, Path};

#[derive(Deserialize)]
struct Manifest {
    events: Vec<ManifestEvent>,
}

#[derive(Deserialize)]
struct ManifestEvent {
    name: String,
}

#[derive(Default)]
struct EventMatchVisitor {
    variants: Vec<String>,
}

impl<'ast> Visit<'ast> for EventMatchVisitor {
    fn visit_expr_match(&mut self, node: &'ast ExprMatch) {
        let is_event_match =
            matches!(node.expr.as_ref(), Expr::Path(path) if path.path.is_ident("event"));
        if is_event_match {
            for arm in &node.arms {
                collect_event_variants(&arm.pat, &mut self.variants);
            }
        }
        visit::visit_expr_match(self, node);
    }
}

fn event_variant(path: &Path) -> Option<String> {
    let mut segments = path.segments.iter().rev();
    let variant = segments.next()?;
    let event = segments.next()?;
    (event.ident == "Event").then(|| variant.ident.to_string())
}

fn collect_event_variants(pattern: &Pat, variants: &mut Vec<String>) {
    match pattern {
        Pat::Path(pattern) => variants.extend(event_variant(&pattern.path)),
        Pat::Struct(pattern) => variants.extend(event_variant(&pattern.path)),
        Pat::TupleStruct(pattern) => variants.extend(event_variant(&pattern.path)),
        Pat::Or(pattern) => {
            for case in &pattern.cases {
                collect_event_variants(case, variants);
            }
        }
        _ => {}
    }
}

#[test]
fn mechanical_bridge_maps_every_manifest_event_once() {
    let syntax = syn::parse_file(include_str!("../src/lib.rs")).expect("seq-bridge source parses");
    let function = syntax
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fn(item) if item.sig.ident == "translate_event" => Some(item),
            _ => None,
        })
        .expect("translate_event exists");
    let mut visitor = EventMatchVisitor::default();
    visitor.visit_block(&function.block);

    let manifest: Manifest =
        serde_json::from_str(include_str!("../../seq-events/event-coverage.json"))
            .expect("event coverage manifest parses");
    let expected: Vec<_> = manifest
        .events
        .into_iter()
        .map(|event| event.name)
        .collect();
    assert_eq!(
        visitor.variants.iter().collect::<BTreeSet<_>>(),
        expected.iter().collect::<BTreeSet<_>>(),
        "bridge match is stale"
    );
    assert_eq!(
        visitor.variants.len(),
        expected.len(),
        "bridge event count is stale"
    );

    let mut counts = std::collections::HashMap::new();
    for name in &visitor.variants {
        *counts.entry(name).or_insert(0usize) += 1;
    }
    assert!(
        counts.values().all(|count| *count == 1),
        "bridge must map every event exactly once"
    );
}
