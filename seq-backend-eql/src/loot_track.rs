//! Turns the loot opcode stream into durable acquisition rows.
//!
//! Stateful because the wire splits one acquisition across two packets: the
//! color-286 narration names the mob and says what happened to the item, and
//! the OP_LootTransaction confirmation that follows carries the authoritative
//! corpse/quantity/coin. A narration is therefore held pending until its
//! confirmation arrives.
//!
//! The ordered `seq-session` owns this tracker. Standalone callers remain only
//! as compatibility adapters while both hosts cut over to semantic events.

use std::collections::{HashMap, HashSet, VecDeque};

/// Chat colour EQL puts personal loot narration on (CC_User_Loot).
pub const LOOT_COLOR: u32 = 286;

/// Where a row came from. `Window` is the corpse's contents (a drop-table
/// view), `Message` is what the player actually acquired, `Coin` is a pile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LootSource {
    Message,
    Window,
    Coin,
}

impl LootSource {
    pub fn as_str(self) -> &'static str {
        match self {
            LootSource::Message => "message",
            LootSource::Window => "window",
            LootSource::Coin => "coin",
        }
    }
}

/// One durable loot row. `0` stands in for SQL NULL on the id/icon columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LootRow {
    pub ts: i64,
    pub source: LootSource,
    pub item_name: String,
    pub item_id: u32,
    pub icon: u32,
    pub qty: u32,
    pub mob_name: String,
    pub mob_norm: String,
    pub corpse_id: u32,
    pub zone_short: String,
    pub zone_base: String,
    pub instance: String,
    pub sold: bool,
    pub money_copper: u32,
    pub disposition: String,
    pub looter: String,
    /// Monotonic loot-request sequence from the confirmation; 0 when the row
    /// has none. Hosts dedup acquisitions on it, since daemon and scry both
    /// record the same capture.
    pub sequence: u32,
    /// `true` when both the narration and confirmation were observed. Corpse
    /// windows and coin piles are complete in one packet. A boundary flushes
    /// either unmatched half with this set to `false`.
    pub complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingConfirmation {
    corpse_id: u32,
    item_id: u32,
    quantity: u32,
    coin_copper: u32,
    sequence: u32,
    ts: i64,
}

/// What a loot narration says happened to the item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedLootLine {
    pub item: String,
    pub qty: u32,
    pub mob: String,
    pub sold: bool,
    /// "inventory" | "sold" | "created" | a storage destination.
    pub disposition: String,
    pub money_copper: u32,
}

const CORPSE_MARK: &str = "'s corpse";

/// Parse a colour-286 loot narration. `None` for anything else on that colour
/// (e.g. "You receive no loot ..."), which the caller ignores.
///
/// Two families, both ending at the corpse marker:
///   `--You have looted <qty?> <item> from <mob>'s corpse.--`   (kept)
///   `You looted <qty?> <item> from <mob>'s corpse<tail>`       (auto)
/// where tail is empty, " and sold it for <money>.", " and stored it in your
/// <dest>", or " to create <upgrade>".
pub fn parse_loot_line(text: &str) -> Option<ParsedLootLine> {
    let t = text.trim();

    let (body, kept) = match t.strip_prefix("--You have looted ") {
        Some(rest) => (rest.strip_suffix("--")?.trim_end(), true),
        None => (t.strip_prefix("You looted ")?, false),
    };

    // Split at the corpse marker: everything before is "<qty?><item> from
    // <mob>", everything after is the disposition tail. rfind so an item or mob
    // name containing the marker cannot end the scan early.
    let mark = body.rfind(CORPSE_MARK)?;
    let (head, tail) = body.split_at(mark);
    let tail = &tail[CORPSE_MARK.len()..];

    // Mob sits after the LAST " from " in the head, so an item name containing
    // " from " does not steal it.
    let from = head.rfind(" from ")?;
    let (count_and_item, mob) = (&head[..from], &head[from + " from ".len()..]);
    if mob.is_empty() {
        return None;
    }

    let (qty, item) = split_leading_count(count_and_item);
    if item.is_empty() {
        return None;
    }

    let (sold, disposition, money_copper) = if kept {
        (false, "inventory".to_string(), 0)
    } else {
        classify_tail(tail)?
    };

    Some(ParsedLootLine {
        item: item.to_string(),
        qty,
        mob: mob.to_string(),
        sold,
        disposition,
        money_copper,
    })
}

/// `(sold, disposition, money)` for an auto-loot tail.
fn classify_tail(tail: &str) -> Option<(bool, String, u32)> {
    let tail = tail.trim_end_matches('.');
    if let Some(money) = tail.strip_prefix(" and sold it for ") {
        return Some((true, "sold".to_string(), parse_money_to_copper(money)));
    }
    if let Some(dest) = tail.strip_prefix(" and stored it in ") {
        let dest = dest.strip_prefix("your ").unwrap_or(dest);
        return Some((false, dest.trim().to_string(), 0));
    }
    if tail.starts_with(" to create ") {
        return Some((false, "created".to_string(), 0));
    }
    if tail == " and destroyed it" {
        return Some((false, "destroyed".to_string(), 0));
    }
    if tail == " and dropped it" || tail == " and dropped it on the ground" {
        return Some((false, "dropped".to_string(), 0));
    }
    if tail.is_empty() {
        return Some((false, "inventory".to_string(), 0));
    }
    None
}

/// Strip a leading count ("2 Bone Chips") or article ("a Fine Steel"),
/// returning `(qty, item)`.
fn split_leading_count(s: &str) -> (u32, &str) {
    let s = s.trim();
    if let Some((first, rest)) = s.split_once(' ') {
        if let Ok(n) = first.parse::<u32>() {
            return (n, rest.trim());
        }
        if first == "a" || first == "an" {
            return (1, rest.trim());
        }
    }
    (1, s)
}

const COIN: [(&str, u32); 4] = [
    ("platinum", 1000),
    ("gold", 100),
    ("silver", 10),
    ("copper", 1),
];

/// Sum "2 platinum, 8 gold, 8 silver and 1 copper" to copper. Order-independent
/// and tolerant of the server's Oxford-comma phrasing.
pub fn parse_money_to_copper(text: &str) -> u32 {
    let mut total = 0u32;
    let mut pending: Option<u32> = None;
    for word in text.split(|c: char| c.is_whitespace() || c == ',') {
        let w = word.trim();
        if w.is_empty() {
            continue;
        }
        if let Ok(n) = w.parse::<u32>() {
            pending = Some(n);
            continue;
        }
        if let Some(n) = pending {
            if let Some((_, mult)) = COIN.iter().find(|(name, _)| *name == w) {
                total = total.saturating_add(n.saturating_mul(*mult));
            }
            pending = None;
        }
    }
    total
}

/// Article-stripped, corpse-suffix-stripped, lowercased — the grouping key.
pub fn normalize_mob(name: &str) -> String {
    let n = name.trim();
    let n = n
        .strip_suffix(CORPSE_MARK)
        .or_else(|| n.strip_suffix(" corpse"))
        .unwrap_or(n);
    let lower = n.trim().to_lowercase();
    for a in ["a ", "an ", "the "] {
        if let Some(rest) = lower.strip_prefix(a) {
            return rest.trim().to_string();
        }
    }
    lower
}

/// EQL encodes raid/solo instances as a zone-name suffix.
const INSTANCE_SUFFIXES: [&str; 3] = ["_eqlraidgroup", "_multi", "_solo"];

pub fn split_zone_instance(zone_short: &str) -> (String, String) {
    for s in INSTANCE_SUFFIXES {
        if let Some(base) = zone_short.strip_suffix(s) {
            return (base.to_string(), s[1..].to_string());
        }
    }
    (zone_short.to_string(), String::new())
}

/// Accumulates loot rows from the opcode stream. Feed it every loot event; it
/// returns rows once they are complete.
#[derive(Debug, Default)]
pub struct LootTracker {
    zone_short: String,
    zone_base: String,
    instance: String,
    looter: String,
    /// Narrations and confirmations can cross in captures. Retain both sides
    /// in wire order and pair by item id when one is available, then FIFO.
    pending_messages: VecDeque<LootRow>,
    pending_confirmations: VecDeque<PendingConfirmation>,
    /// `corpse_id\u{1}item` already recorded from a window, so reopening a
    /// corpse does not double-count.
    seen_window: HashSet<String>,
    /// Full ordered corpse windows already surfaced as semantic snapshots.
    seen_window_snapshots: HashSet<String>,
    /// Learned from window rows; backfills a narration that carried no link.
    item_id_by_name: HashMap<String, u32>,
    /// A nonzero server sequence is authoritative. Sequence-less fixtures use
    /// all transaction fields plus the capture timestamp as their identity.
    seen_confirmations: HashSet<String>,
    /// Suppress a replayed copy of the exact same application message without
    /// collapsing legitimate repeated acquisitions at different timestamps.
    seen_messages: HashSet<String>,
}

impl LootTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Drop all state (zone change to an unknown zone, session restart).
    pub fn reset(&mut self) {
        *self = Self {
            looter: std::mem::take(&mut self.looter),
            ..Self::default()
        };
    }

    pub fn set_looter(&mut self, looter: &str) {
        self.looter = looter.to_string();
    }

    /// Current full zone name, including any EQL instance suffix.
    pub fn zone(&self) -> &str {
        &self.zone_short
    }

    /// Zone-in. Flushes any narration that never got its confirmation, so the
    /// row keeps the zone it was looted in.
    pub fn set_zone(&mut self, zone_short: &str) -> Vec<LootRow> {
        let out = self.flush();
        if !zone_short.is_empty() {
            if self.zone_short != zone_short {
                self.reset();
            }
            let (base, instance) = split_zone_instance(zone_short);
            self.zone_short = zone_short.to_string();
            self.zone_base = base;
            self.instance = instance;
        }
        out
    }

    /// A colour-286 narration. `item_id`/`item_name` come off the link header
    /// and win over the prose when present.
    pub fn on_loot_message(
        &mut self,
        color: u32,
        text: &str,
        item_id: u32,
        item_name: &str,
        ts: i64,
    ) -> Vec<LootRow> {
        if color != LOOT_COLOR {
            return Vec::new();
        }
        let Some(p) = parse_loot_line(text) else {
            return Vec::new();
        };
        let name = if item_name.is_empty() {
            p.item.clone()
        } else {
            item_name.to_string()
        };
        let id = if item_id != 0 {
            item_id
        } else {
            self.item_id_by_name.get(&name).copied().unwrap_or(0)
        };

        let message_key = format!("{ts}\u{1}{item_id}\u{1}{item_name}\u{1}{text}");
        if !self.seen_messages.insert(message_key) {
            return Vec::new();
        }

        let mut row = LootRow {
            ts,
            source: LootSource::Message,
            item_name: name,
            item_id: id,
            icon: 0,
            qty: p.qty,
            mob_norm: normalize_mob(&p.mob),
            mob_name: p.mob,
            corpse_id: 0,
            zone_short: self.zone_short.clone(),
            zone_base: self.zone_base.clone(),
            instance: self.instance.clone(),
            sold: p.sold,
            money_copper: p.money_copper,
            disposition: p.disposition,
            looter: self.looter.clone(),
            sequence: 0,
            complete: false,
        };
        if let Some(index) = matching_confirmation(&self.pending_confirmations, row.item_id) {
            let confirmation = self
                .pending_confirmations
                .remove(index)
                .expect("matching confirmation index");
            apply_confirmation(&mut row, confirmation);
            vec![row]
        } else {
            self.pending_messages.push_back(row);
            Vec::new()
        }
    }

    /// A loot confirmation. `from_corpse` is the coin pile, which names no item
    /// and so must NOT consume the pending narration — it arrives at
    /// loot-window open, ahead of the item lines.
    #[allow(clippy::too_many_arguments)]
    pub fn on_loot_transaction(
        &mut self,
        corpse_id: u32,
        item_id: u32,
        quantity: u32,
        coin_copper: u32,
        from_corpse: bool,
        sequence: u32,
        ts: i64,
    ) -> Vec<LootRow> {
        let dedup_key = if sequence != 0 {
            format!("sequence:{sequence}")
        } else {
            format!("fields:{corpse_id}:{item_id}:{quantity}:{coin_copper}:{from_corpse}:{ts}")
        };
        if !self.seen_confirmations.insert(dedup_key) {
            return Vec::new();
        }

        if from_corpse {
            if coin_copper == 0 {
                return Vec::new();
            }
            return vec![LootRow {
                ts,
                source: LootSource::Coin,
                item_name: "Coin".to_string(),
                item_id: 0,
                icon: 0,
                qty: 1,
                mob_name: String::new(),
                mob_norm: String::new(),
                corpse_id,
                zone_short: self.zone_short.clone(),
                zone_base: self.zone_base.clone(),
                instance: self.instance.clone(),
                sold: false,
                money_copper: coin_copper,
                disposition: "corpse_coin".to_string(),
                looter: self.looter.clone(),
                sequence,
                complete: true,
            }];
        }

        let confirmation = PendingConfirmation {
            corpse_id,
            item_id,
            quantity,
            coin_copper,
            sequence,
            ts,
        };
        let Some(index) = matching_message(&self.pending_messages, item_id) else {
            self.pending_confirmations.push_back(confirmation);
            return Vec::new();
        };
        let mut row = self
            .pending_messages
            .remove(index)
            .expect("matching message index");
        apply_confirmation(&mut row, confirmation);
        vec![row]
    }

    /// One item from a corpse's loot window: the drop-table view. Dedups per
    /// corpse+item so reopening a corpse does not double-count, and teaches the
    /// item id so a later narration carrying no link can still resolve it.
    ///
    /// Per item rather than per window because the cxx bridge cannot pass a
    /// slice of tuples; both hosts already iterate the window's items anyway.
    pub fn on_loot_drop_item(
        &mut self,
        corpse_id: u32,
        corpse_name: &str,
        item_name: &str,
        icon: u32,
        item_id: u32,
        ts: i64,
    ) -> Vec<LootRow> {
        if item_id != 0 {
            self.item_id_by_name.insert(item_name.to_string(), item_id);
        }
        if !self
            .seen_window
            .insert(format!("{corpse_id}\u{1}{item_name}"))
        {
            return Vec::new();
        }
        vec![LootRow {
            ts,
            source: LootSource::Window,
            item_name: item_name.to_string(),
            item_id,
            icon,
            qty: 1,
            mob_name: corpse_name.to_string(),
            mob_norm: normalize_mob(corpse_name),
            corpse_id,
            zone_short: self.zone_short.clone(),
            zone_base: self.zone_base.clone(),
            instance: self.instance.clone(),
            sold: false,
            money_copper: 0,
            disposition: String::new(),
            looter: self.looter.clone(),
            sequence: 0,
            complete: true,
        }]
    }

    /// Return true once for each distinct ordered corpse-window snapshot.
    pub fn observe_window_snapshot(
        &mut self,
        corpse_id: u32,
        corpse_name: &str,
        items: &[seq_events::LootItemInfo],
    ) -> bool {
        let mut key = format!("{corpse_id}\u{1}{corpse_name}");
        for item in items {
            use std::fmt::Write as _;
            let _ = write!(
                key,
                "\u{1}{}\u{1}{}\u{1}{}",
                item.item_id, item.icon, item.name
            );
        }
        self.seen_window_snapshots.insert(key)
    }

    /// Emit every unmatched half at an ordered boundary. A confirmation-only
    /// row deliberately retains the authoritative ids, quantity, proceeds,
    /// sequence, and timestamp even though no narration supplied a name.
    pub fn flush(&mut self) -> Vec<LootRow> {
        let mut rows: Vec<_> = self.pending_messages.drain(..).collect();
        rows.extend(
            self.pending_confirmations
                .drain(..)
                .map(|confirmation| LootRow {
                    ts: confirmation.ts,
                    source: LootSource::Message,
                    item_name: String::new(),
                    item_id: confirmation.item_id,
                    icon: 0,
                    qty: confirmation.quantity.max(1),
                    mob_name: String::new(),
                    mob_norm: String::new(),
                    corpse_id: confirmation.corpse_id,
                    zone_short: self.zone_short.clone(),
                    zone_base: self.zone_base.clone(),
                    instance: self.instance.clone(),
                    sold: confirmation.coin_copper != 0,
                    money_copper: confirmation.coin_copper,
                    disposition: if confirmation.coin_copper == 0 {
                        String::new()
                    } else {
                        "sold".to_string()
                    },
                    looter: self.looter.clone(),
                    sequence: confirmation.sequence,
                    complete: false,
                }),
        );
        rows.sort_by_key(|row| row.ts);
        rows
    }
}

fn matching_confirmation(
    confirmations: &VecDeque<PendingConfirmation>,
    item_id: u32,
) -> Option<usize> {
    if item_id != 0 {
        if let Some(index) = confirmations
            .iter()
            .position(|confirmation| confirmation.item_id == item_id)
        {
            return Some(index);
        }
        return confirmations
            .iter()
            .position(|confirmation| confirmation.item_id == 0);
    }
    (!confirmations.is_empty()).then_some(0)
}

fn matching_message(messages: &VecDeque<LootRow>, item_id: u32) -> Option<usize> {
    if item_id != 0 {
        if let Some(index) = messages
            .iter()
            .position(|message| message.item_id == item_id)
        {
            return Some(index);
        }
        return messages.iter().position(|message| message.item_id == 0);
    }
    (!messages.is_empty()).then_some(0)
}

fn apply_confirmation(row: &mut LootRow, confirmation: PendingConfirmation) {
    if confirmation.item_id != 0 {
        row.item_id = confirmation.item_id;
    }
    if confirmation.corpse_id != 0 {
        row.corpse_id = confirmation.corpse_id;
    }
    if confirmation.quantity != 0 {
        row.qty = confirmation.quantity;
    }
    row.money_copper = confirmation.coin_copper;
    row.sequence = confirmation.sequence;
    row.complete = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    // Verbatim from eqlegends-loot2 / eqlegends-loot (2026-08-08).
    const SOLD_2G: &str =
        "You looted a Bronze Dagger +1 from a goblin diviner's corpse and sold it for 2 gold.";
    const SOLD_MIXED: &str = "You looted a Cloth Veil +1 from a goblin diviner's corpse and sold it for 1 gold, 1 silver and 4 copper.";
    const DEPOT: &str = "You looted 2 Bone Chips from a decaying skeleton's corpse and stored it in your tradeskill depot";
    const HOARD: &str =
        "You looted a Diamond Dust from an ice giant's corpse and stored it in your Dragon Hoard";
    const CREATED: &str = "You looted a Throwing Boulder from an ice giant diplomat's corpse to create a Throwing Boulder +8";
    const DROPPED: &str =
        "You looted a Rusty Sword from a goblin's corpse and dropped it on the ground.";
    const DESTROYED: &str = "You looted a Rusty Mace from an orc's corpse and destroyed it.";
    const KEPT: &str = "--You have looted a Dragon Bone Bracelet from Lady Vox's corpse.--";
    const KEPT_QTY: &str = "--You have looted 2 Bone Chips from a cracked skeleton's corpse.--";

    #[test]
    fn parses_a_sale() {
        let p = parse_loot_line(SOLD_2G).unwrap();
        assert_eq!(p.item, "Bronze Dagger +1");
        assert_eq!(p.mob, "a goblin diviner");
        assert_eq!(p.qty, 1);
        assert!(p.sold);
        assert_eq!(p.disposition, "sold");
        assert_eq!(p.money_copper, 200);
    }

    #[test]
    fn parses_a_mixed_denomination_sale() {
        let p = parse_loot_line(SOLD_MIXED).unwrap();
        assert_eq!(p.money_copper, 114);
        assert_eq!(p.item, "Cloth Veil +1");
    }

    #[test]
    fn parses_every_disposition() {
        for (line, item, disp, qty) in [
            (DEPOT, "Bone Chips", "tradeskill depot", 2),
            (HOARD, "Diamond Dust", "Dragon Hoard", 1),
            (CREATED, "Throwing Boulder", "created", 1),
            (DROPPED, "Rusty Sword", "dropped", 1),
            (DESTROYED, "Rusty Mace", "destroyed", 1),
            (KEPT, "Dragon Bone Bracelet", "inventory", 1),
            (KEPT_QTY, "Bone Chips", "inventory", 2),
        ] {
            let p = parse_loot_line(line).unwrap_or_else(|| panic!("no match: {line}"));
            assert_eq!(p.item, item, "{line}");
            assert_eq!(p.disposition, disp, "{line}");
            assert_eq!(p.qty, qty, "{line}");
            assert_eq!(p.money_copper, 0, "{line}");
        }
    }

    #[test]
    fn mob_keeps_its_article_but_norm_does_not() {
        let p = parse_loot_line(HOARD).unwrap();
        assert_eq!(p.mob, "an ice giant");
        assert_eq!(normalize_mob(&p.mob), "ice giant");
        assert_eq!(normalize_mob("Lady Vox"), "lady vox");
        assert_eq!(
            normalize_mob("a decaying skeleton's corpse"),
            "decaying skeleton"
        );
    }

    #[test]
    fn ignores_other_colour_286_lines() {
        assert!(parse_loot_line("You receive no loot from that corpse.").is_none());
        assert!(parse_loot_line("You cannot see your target.").is_none());
        assert!(parse_loot_line("").is_none());
    }

    #[test]
    fn money_parsing_is_order_and_comma_tolerant() {
        assert_eq!(
            parse_money_to_copper("2 platinum, 8 gold, 8 silver and 1 copper"),
            2881
        );
        assert_eq!(parse_money_to_copper("7 silver and 1 copper"), 71);
        assert_eq!(parse_money_to_copper("2 gold"), 200);
        assert_eq!(parse_money_to_copper("nothing at all"), 0);
    }

    #[test]
    fn splits_the_instance_suffix() {
        assert_eq!(
            split_zone_instance("permafrost_eqlraidgroup"),
            ("permafrost".to_string(), "eqlraidgroup".to_string())
        );
        assert_eq!(
            split_zone_instance("greatdivide"),
            ("greatdivide".to_string(), String::new())
        );
    }

    fn tracker_in_zone() -> LootTracker {
        let mut t = LootTracker::new();
        t.set_zone("permafrost_multi");
        t
    }

    #[test]
    fn pairs_a_narration_with_its_confirmation() {
        let mut t = tracker_in_zone();
        assert!(t
            .on_loot_message(LOOT_COLOR, SOLD_2G, 7012, "Bronze Dagger +1", 100)
            .is_empty());
        let rows = t.on_loot_transaction(18632, 0, 1, 200, false, 238, 101);
        assert_eq!(rows.len(), 1);
        let r = &rows[0];
        assert_eq!(r.item_name, "Bronze Dagger +1");
        assert_eq!(r.item_id, 7012); // from the link, not the confirmation
        assert_eq!(r.corpse_id, 18632);
        assert_eq!(r.money_copper, 200);
        assert_eq!(r.sequence, 238);
        assert!(r.complete);
        assert_eq!(r.zone_base, "permafrost");
        assert_eq!(r.instance, "multi");
        assert!(r.sold);
    }

    #[test]
    fn a_corpse_pile_does_not_consume_a_pending_narration() {
        // The pile arrives at window-open, ahead of the item lines.
        let mut t = tracker_in_zone();
        t.on_loot_message(LOOT_COLOR, SOLD_2G, 7012, "Bronze Dagger +1", 100);
        let coin = t.on_loot_transaction(18632, 0, 0, 2881, true, 0, 101);
        assert_eq!(coin.len(), 1);
        assert_eq!(coin[0].source, LootSource::Coin);
        assert_eq!(coin[0].money_copper, 2881);
        // The narration is still pending and still pairs correctly.
        let rows = t.on_loot_transaction(18632, 0, 1, 200, false, 238, 102);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].money_copper, 200);
    }

    #[test]
    fn a_coinless_pile_records_nothing() {
        let mut t = tracker_in_zone();
        assert!(t.on_loot_transaction(1, 0, 0, 0, true, 0, 100).is_empty());
    }

    #[test]
    fn an_orphan_confirmation_is_incomplete_at_the_boundary() {
        let mut t = tracker_in_zone();
        assert!(t
            .on_loot_transaction(1, 2, 1, 500, false, 9, 100)
            .is_empty());
        let rows = t.flush();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].item_id, 2);
        assert_eq!(rows[0].corpse_id, 1);
        assert_eq!(rows[0].sequence, 9);
        assert!(!rows[0].complete);
    }

    #[test]
    fn multiple_narrations_wait_and_flush_in_capture_order() {
        let mut t = tracker_in_zone();
        t.on_loot_message(LOOT_COLOR, KEPT, 0, "", 100);
        assert!(t.on_loot_message(LOOT_COLOR, HOARD, 0, "", 101).is_empty());
        let flushed = t.flush();
        assert_eq!(flushed.len(), 2);
        assert_eq!(flushed[0].item_name, "Dragon Bone Bracelet");
        assert_eq!(flushed[1].item_name, "Diamond Dust");
        assert!(flushed.iter().all(|row| !row.complete));
    }

    #[test]
    fn confirmation_before_narration_pairs_by_item_id() {
        let mut t = tracker_in_zone();
        assert!(t
            .on_loot_transaction(18_632, 7012, 1, 200, false, 238, 99)
            .is_empty());
        let rows = t.on_loot_message(LOOT_COLOR, SOLD_2G, 7012, "Bronze Dagger +1", 100);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].corpse_id, 18_632);
        assert_eq!(rows[0].sequence, 238);
        assert_eq!(rows[0].ts, 100);
        assert!(rows[0].complete);
    }

    #[test]
    fn known_item_ids_do_not_cross_pair_when_interleaved() {
        let mut t = tracker_in_zone();
        t.on_loot_transaction(1, 7012, 1, 200, false, 1, 90);
        t.on_loot_transaction(2, 16884, 1, 0, false, 2, 91);

        let diamond = t.on_loot_message(LOOT_COLOR, HOARD, 16884, "Diamond Dust", 100);
        assert_eq!(diamond[0].corpse_id, 2);
        let dagger = t.on_loot_message(LOOT_COLOR, SOLD_2G, 7012, "Bronze Dagger +1", 101);
        assert_eq!(dagger[0].corpse_id, 1);
    }

    #[test]
    fn duplicate_messages_confirmations_and_windows_emit_once() {
        let mut t = tracker_in_zone();
        t.on_loot_message(LOOT_COLOR, SOLD_2G, 7012, "Bronze Dagger +1", 100);
        assert!(t
            .on_loot_message(LOOT_COLOR, SOLD_2G, 7012, "Bronze Dagger +1", 100)
            .is_empty());
        assert_eq!(
            t.on_loot_transaction(18_632, 7012, 1, 200, false, 238, 101)
                .len(),
            1
        );
        assert!(t
            .on_loot_transaction(18_632, 7012, 1, 200, false, 238, 102)
            .is_empty());
        assert_eq!(
            t.on_loot_drop_item(18_632, "a goblin", "Rusty Sword", 1, 2, 103)
                .len(),
            1
        );
        assert!(t
            .on_loot_drop_item(18_632, "a goblin", "Rusty Sword", 1, 2, 104)
            .is_empty());
    }

    #[test]
    fn reset_discards_all_pairing_and_duplicate_state() {
        let mut t = tracker_in_zone();
        t.on_loot_transaction(18_632, 7012, 1, 200, false, 238, 99);
        t.on_loot_drop_item(18_632, "a goblin", "Rusty Sword", 1, 2, 100);
        t.reset();

        assert!(t
            .on_loot_message(LOOT_COLOR, SOLD_2G, 7012, "Bronze Dagger +1", 101)
            .is_empty());
        assert_eq!(t.flush().len(), 1, "pre-reset confirmation cannot pair");
        assert_eq!(
            t.on_loot_drop_item(18_632, "a goblin", "Rusty Sword", 1, 2, 102)
                .len(),
            1,
            "window duplicate state must not survive"
        );
    }

    #[test]
    fn window_rows_dedup_per_corpse_and_teach_item_ids() {
        let mut t = tracker_in_zone();
        let row = t.on_loot_drop_item(11613, "an ice giant", "Diamond Dust", 1075, 16884, 100);
        assert_eq!(row.len(), 1);
        assert_eq!(row[0].source, LootSource::Window);
        assert_eq!(row[0].icon, 1075);
        // Reopening the same corpse re-sends the list.
        assert!(t
            .on_loot_drop_item(11613, "an ice giant", "Diamond Dust", 1075, 16884, 101)
            .is_empty());
        // A different corpse with the same item is a distinct row.
        assert_eq!(
            t.on_loot_drop_item(11614, "an ice giant", "Diamond Dust", 1075, 16884, 102)
                .len(),
            1
        );
        // A narration with no link picks the id up from the window.
        t.on_loot_message(LOOT_COLOR, HOARD, 0, "", 103);
        assert_eq!(t.flush()[0].item_id, 16884);
    }

    #[test]
    fn zone_change_flushes_with_the_old_zone() {
        let mut t = tracker_in_zone();
        t.on_loot_message(LOOT_COLOR, KEPT, 0, "", 100);
        t.on_loot_drop_item(1, "a goblin", "Rusty Sword", 2, 3, 100);
        t.on_loot_transaction(1, 4, 1, 0, false, 42, 100);
        t.on_loot_message(LOOT_COLOR, HOARD, 0, "", 101);
        let rows = t.set_zone("greatdivide");
        assert_eq!(rows.len(), 1);
        assert!(rows.iter().all(|row| row.zone_base == "permafrost"));
        assert_eq!(
            t.on_loot_drop_item(1, "a goblin", "Rusty Sword", 2, 3, 101)
                .len(),
            1,
            "window state must not cross zones"
        );
        assert!(t.on_loot_transaction(1, 4, 1, 0, false, 42, 102).is_empty());
        assert_eq!(t.flush().len(), 1, "sequence state must not cross zones");
    }

    #[test]
    fn non_loot_colours_are_ignored() {
        let mut t = tracker_in_zone();
        assert!(t.on_loot_message(2, SOLD_2G, 0, "", 100).is_empty());
        assert!(t.flush().is_empty());
    }
}
