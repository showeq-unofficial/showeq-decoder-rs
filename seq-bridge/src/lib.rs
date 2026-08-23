//! C++ FFI bridge — exposes the Rust decoders across the cxx ABI.
//!
//! The bridge is intentionally a thin shim. Parsing logic stays in the decode
//! crates (`seq-decode` for the shared/Live path, `seq-backend-eql` for eql's
//! diverged opcodes) so it remains usable from pure-Rust contexts (replay
//! tools, the eventual standalone daemon). This crate is the `staticlib`
//! Corrosion links into `seq-daemon-core`.
//!
//! Backend selection via Cargo features: `backend-live` / `backend-test` pick
//! the bindings crate in `seq-decode` (Test rides the shared Live decoders).
//! `backend-eql` instead links `seq-backend-eql` — a fully self-contained eql
//! decode stack that shares NOTHING with Live, so a Live wire patch can't reach
//! eql. The uniform `decode_*` FFI surface is identical for every backend; the
//! `backend` alias below routes each call to the active backend's decoders, and
//! the few opcodes whose eql wire diverges call eql's parser explicitly.

#[cfg(not(any(
    feature = "backend-live",
    feature = "backend-test",
    feature = "backend-eql"
)))]
compile_error!("seq-bridge: enable exactly one backend feature (default: backend-live).");

// The active backend's decoder crate. live/test share `seq-decode`; eql is its
// own self-contained stack (`seq-backend-eql`). Shared decoders call
// `backend::parse_*`; the eql-diverged opcodes cfg-select
// `seq_backend_eql::parse_legends_*` directly below.
#[cfg(feature = "backend-eql")]
use seq_backend_eql as backend;
#[cfg(not(feature = "backend-eql"))]
use seq_decode as backend;

// cxx expands `-> Box<T>` entries into code using an API stabilized after the
// workspace's declared rust-version (1.75). It's generated, so there is nothing
// here to rewrite; the toolchain we actually build with is `stable`.
#[allow(clippy::incompatible_msrv)]
#[cxx::bridge(namespace = "seq::rust")]
mod ffi {
    /// Decoded `OP_MobUpdate` payload. `ok` is the discriminator: when
    /// false the remaining fields are zeroed and the daemon drops the
    /// packet. The discriminator is preferred over cxx's `Result`
    /// mapping because the latter would emit C++ exceptions for what
    /// the daemon's SZC_Match dispatch already prevents.
    struct MobUpdate {
        spawn_id: u16,
        x: i32,
        y: i32,
        z: i32,
        heading: u16,
        ok: bool,
    }

    /// Decoded `OP_DeleteSpawn`. `ok=false` means the payload was the
    /// wrong size; SZC_Match in the daemon already guards against that
    /// so it shouldn't fire in normal operation.
    struct DeleteSpawn {
        spawn_id: u32,
        ok: bool,
    }

    /// Variable-length spawn payload (Stage A+2). Mirrors the fields
    /// `SpawnShell::fillSpawnStruct` populates on a `spawnStruct`. The
    /// daemon assigns each field into its own struct via `applySpawn`.
    /// `bytes_consumed` records the parser's consumed length.
    struct Spawn {
        ok: bool,
        bytes_consumed: u32,

        // Daemon strncpy's into its fixed-size char buffers
        // (everquest.h spawnStruct).
        name: String,
        last_name: String,
        title: String,
        suffix: String,

        spawn_id: u32,
        misc_data: u32,
        body_type: u32,
        race: u32,
        deity: u32,
        guild_id: u32,
        guild_server_id: u32,
        class_: u32,
        class_mask: u32,
        pet_owner_id: u32,

        // 9 slots × 5 u32s — same memory layout as
        // EquipStruct equipment[9] on the C++ side.
        equip_data: [u32; 45],
        pos_data: [u32; 5],

        level: u8,
        npc: u8,
        other_data: u8,
        char_properties: u8,
        cur_hp: u8,
        holding: u8,
        state: u8,
        light: u8,
        is_mercenary: u8,

        // Decoded position + hp, filled only by backends whose spawn wire is
        // already decoded (eql). Live leaves these zero and uses pos_data /
        // cur_hp; eql leaves the raw arrays zero and fills these.
        x: i16,
        y: i16,
        z: i16,
        max_hp: u8,
        // Decoded h2048 heading (0..2047). eql only; Live leaves 0 and takes
        // heading from pos_data / OP_MobUpdate.
        heading: u16,
    }

    // Stage A+3 — small fixed-size opcodes. Each struct ends with an
    // `ok` discriminator: false means the payload was malformed (length
    // mismatch / failed extraction); the daemon drops the packet.
    struct RemoveSpawn {
        spawn_id: u32,
        remove_spawn: u8,
        ok: bool,
    }
    struct HpUpdate {
        spawn_id: u16,
        cur_hp: i32,
        max_hp: i32,
        ok: bool,
    }
    // eql OP_HPUpdate is a multiplexed stat-sync channel, not Live's
    // fixed HP struct — decoded via decode_stat_sync. `wide` gates the real
    // cur/max (vs percent) forms; `has_*` mark which stats the packet carried.
    struct StatSync {
        spawn_id: u32,
        wide: bool,
        has_hp: bool,
        hp_cur: i64,
        hp_max: i64,
        has_mana: bool,
        mana_cur: i64,
        mana_max: i64,
        has_end: bool,
        end_cur: i64,
        end_max: i64,
        ok: bool,
    }
    // One stat-sync packet's verdict from EqlSelfTracker. `is_self` false means
    // the packet belongs to another spawn and the host routes its HP normally;
    // the has_* flags are meaningful only when is_self is true.
    struct SelfStat {
        is_self: bool,
        has_hp: bool,
        hp_cur: i64,
        hp_max: i64,
        has_mana: bool,
        mana_cur: i64,
        mana_max: i64,
        has_end: bool,
        end_cur: i64,
        end_max: i64,
    }
    // eql OP_BeginCast (S>C): a spawn started casting. spell_id/caster_id
    // are resolved to names daemon-side; cast_time_ms drives the web cast timer.
    struct BeginCast {
        caster_id: u32,
        spell_id: u32,
        cast_time_ms: u32,
        ok: bool,
    }
    // eql OP_Stance / OP_Invocation, S>C: 4B {u32 abilityId}.
    // The daemon resolves ability_id to a display name (stance vs invocation
    // table picked by the opcode). ok=false = wrong-size payload.
    struct ActivateAbility {
        ability_id: u32,
        ok: bool,
    }
    // eql OP_SendAATable (S>C): one AA ability-rank definition per packet
    // (burst at zone-in). desc_id == the profile's per-rank aa id; title_sid is a
    // dbstr type-1 id the daemon resolves to the AA display name. ok=false = short.
    struct AaTableEntry {
        desc_id: u32,
        title_sid: u32,
        ok: bool,
    }
    // eql OP_LoadoutSwap: a player's multiclass loadout change. Only
    // the identity fields that change on a swap are surfaced; spawn_id is the
    // header id of the player who swapped (self or a nearby tracked spawn).
    struct LoadoutSwap {
        spawn_id: u32,
        level: u8,
        class_: u32,
        race: u32,
        ok: bool,
    }
    struct LootTransaction {
        item_id: u32,
        slot: u32,
        corpse_id: u32,
        quantity: u32,
        sequence: u32,
        coin_copper: u32,
        // Subcode-5 corpse pile; the item fields are 0.
        from_corpse: bool,
        ok: bool,
    }
    // eql OP_LootMessage: the personal loot narration. item_id/item_name come
    // off the link header (0/empty when the line carries no item link) and are
    // authoritative — a consumer never has to recover the item from the prose.
    struct LootMessage {
        color: u32,
        text: String,
        item_id: u32,
        item_name: String,
        ok: bool,
    }
    // One durable loot row from EqlLootTracker. `source` is "message" (what the
    // player acquired), "window" (corpse contents) or "coin" (a pile); 0 stands
    // in for SQL NULL on the id/icon columns. `sequence` is the confirmation's
    // monotonic counter — the host dedups acquisitions on it, since more than
    // one recorder may be watching the same capture.
    struct LootRow {
        ts: i64,
        source: String,
        item_name: String,
        item_id: u32,
        icon: u32,
        qty: u32,
        mob_name: String,
        mob_norm: String,
        corpse_id: u32,
        zone_short: String,
        zone_base: String,
        instance: String,
        sold: bool,
        money_copper: u32,
        disposition: String,
        looter: String,
        sequence: u32,
    }
    struct MoneyUpdate {
        platinum: u32,
        gold: u32,
        silver: u32,
        copper: u32,
        ok: bool,
    }
    struct LootItem {
        name: String,
        icon: u32,
        item_id: u32,
    }
    struct LootDrops {
        corpse_id: u32,
        corpse_name: String,
        items: Vec<LootItem>,
        ok: bool,
    }
    struct MobHealth {
        spawn_id: u16,
        hp_percent: i32,
        ok: bool,
    }
    // Widened to u32 for Live's re-derived 8-byte `{u32 spawnId, u32 type}`
    // layout (2026-07-28); eql's own struct is still u16/u16/u32 and converts
    // losslessly. `parameter` survives for eql — Live's wire has no value field.
    struct SpawnAppearance {
        spawn_id: u32,
        kind: u32,
        parameter: u32,
        ok: bool,
    }
    struct ExpUpdate {
        exp: u32,
        unknown0: u32,
        kind: u32,
        unknown1: u32,
        ok: bool,
    }
    struct LevelUpdate {
        level: u32,
        level_old: u32,
        exp: u32,
        unknown0: u32,
        ok: bool,
    }
    struct SkillUpdate {
        skill_id: u32,
        value: i32,
        ok: bool,
    }

    // Stage A+4 — 7 more small fixed-size opcodes.
    struct ManaChange {
        new_mana: i32,
        max_mana: i32,
        spell_id: i32,
        ok: bool,
    }
    struct Stamina {
        food: u32,
        water: u32,
        ok: bool,
    }
    struct EndUpdate {
        spawn_id: u16,
        cur: u32,
        max: u32,
        ok: bool,
    }
    struct Consider {
        player_id: u32,
        target_id: u32,
        faction: i32,
        level: i32,
        ok: bool,
    }
    /// A backend-declared payload size override: `name` (a toml `typename`) →
    /// its authoritative size on the linked backend. The daemon applies these
    /// over its C++ `sizeof` size table so `SZC_Match` validates against the
    /// backend's size, not a hardcoded Live `sizeof`. Empty for live/test.
    struct StructSize {
        name: String,
        size: u32,
    }
    // eql OP_GuildMOTD — the guild message of the day. Single struct (not a list);
    // `ok` false = decode failed / too short. The packet carries no guild id (the
    // MOTD is implicitly the local player's guild), so none is returned.
    struct GuildMotd {
        message: String,
        sender: String,
        ok: bool,
    }
    // OP_ExpandedGuildInfo (Live) — one entry of the guild rank-name table.
    // The opcode is a tagged union; `action` selects the shape. Only the
    // rank-name action (3) fills rank_index (1-based, matches the roster member
    // rank field) + rank_name; other actions leave them 0/empty. The consumer
    // gates on `action == 3` and builds a rank -> name table for the guild.
    struct GuildExpandedInfo {
        action: u32,
        guild_id: u32,
        rank_index: u32,
        rank_name: String,
    }
    // OP_GuildMemberUpdate (Live) — one member's zone/last-on update (NOT rank).
    // `ok` false = decode failed. Identified by `name`; the consumer updates that
    // roster member's online state. zone_id 0 = offline.
    struct GuildMemberUpdateInfo {
        name: String,
        zone_id: u16,
        zone_instance: u16,
        last_on: u32,
        ok: bool,
    }
    // One guild present in the zone (eql OP_GuildsInZoneList / OP_NewGuildInZone).
    // Returned as a flat Vec — the list opcode yields N, the single opcode yields
    // one — so the daemon feeds both through the same GuildMgr primitive.
    struct GuildInZoneRow {
        guild_id: u32,
        server_id: u32,
        name: String,
    }
    // One row of the eql guild roster (OP_GuildMemberList). Returned as a flat
    // Vec (empty on decode failure); `guild_id` repeats per row so the C++ side
    // needs no wrapper struct, mirroring BuffListEntry.
    //
    // `class_mask` is the eql MULTICLASS BITMASK (bit N = class N), not a class
    // id — a character has three simultaneous classes. `primary_class` is its
    // lowest set bit, for a consumer that can show only one.
    // `zone_id` 0 = offline; `last_on` is unix seconds, 0 = never.
    struct GuildRosterRow {
        guild_id: u32,
        name: String,
        level: u32,
        class_mask: u32,
        primary_class: u8,
        rank: u32,
        last_on: u32,
        banker: u8,
        alt: u8,
        full_member: u8,
        public_note: String,
        zone_id: u16,
    }
    // One record of eql OP_BuffList. Returned as a flat Vec (empty on
    // decode failure); every entry repeats spawn_id so the C++ side can filter
    // to the player without a wrapper struct. remaining_ticks <= 0 = permanent.
    struct BuffListEntry {
        spawn_id: u32,
        spell_id: u32,
        remaining_ticks: i32,
        slot: u32,
        // Who cast it. A non-self owner's list mixes the spawn's own buffs
        // with the ones the player put on it; only this tells them apart.
        caster: String,
    }
    // One point in the EQL self-position breadcrumb (OP_SelfPosEQL). Game
    // coords (not screen-negated); `ts` is a per-sample monotonic timer. Ordered
    // oldest -> newest. An empty Vec = decode failed / not the breadcrumb.
    struct SelfPosPoint {
        x: f32,
        y: f32,
        z: f32,
        ts: u32,
    }
    // One EQL UCS (cross-zone chat) line. `channel_first` is the still-masked
    // first byte of the channel name; `channel_rest` is the clean remainder.
    // The caller recovers the per-session mask (from the General* crib) to
    // repair `channel_first`. An empty Vec = no chat in the packet.
    struct UcsChatRecord {
        channel_first: u8,
        channel_rest: String,
        channel_run: String,
        sender: String,
        message: String,
        spam: bool,
    }
    struct SpawnRename {
        old_name: String,
        old_name_again: String,
        new_name: String,
        ok: bool,
    }
    struct ClientTarget {
        new_target: u32,
        ok: bool,
    }
    struct Death {
        spawn_id: u32,
        killer_id: u32,
        corpse_id: u32,
        kind: i32,
        spell_id: u32,
        zone_id: u16,
        zone_instance: u16,
        damage: u32,
        ok: bool,
    }

    // Stage A+5
    struct ClickObject {
        drop_id: u16,
        spawn_id: u16,
        ok: bool,
    }
    struct Illusion {
        spawn_id: u32,
        name: String,
        race: u32,
        gender: u8,
        texture: u8,
        helm: u8,
        face: u32,
        ok: bool,
    }
    struct Buff {
        spawn_id: u32,
        spell_id: u32,
        // form: 0=fade(13b) | 1=initial(30b) | 2=live-update(34+b) |
        // 3=compact(24b, eql buff-slot channel). For form 3, spawn_id is 0 and
        // slot is 0xff for scribe/bar refreshes; change_type is 1=faded/4=applied.
        form: u8,
        slot: u8,
        dur_ticks: u32,
        change_type: u32,
        ok: bool,
    }
    struct Action2 {
        target: u16,
        source: u16,
        damage: i32,
        spell: i32,
        kind: u8,
        ok: bool,
    }

    // Stage A+6 — second small-fixed POD batch.
    struct WearChange {
        spawn_id: u16,
        subcommand: u16,
        arg1: i16,
        arg2: i16,
        arg3: u8,
        ok: bool,
    }
    struct ZoneChange {
        name: String,
        zone_id: u16,
        zone_instance: u16,
        ok: bool,
    }
    struct DzInfo {
        new_dz: u8,
        max_players: u32,
        dz_name: String,
        name: String,
        ok: bool,
    }
    struct DzSwitch {
        zone_id: u16,
        instance_id: u16,
        kind: u32,
        x: f32,
        y: f32,
        z: f32,
        ok: bool,
    }
    struct StartCast {
        slot: i32,
        spell_id: u32,
        target_id: u32,
        ok: bool,
    }
    struct Action {
        target: u16,
        source: u16,
        spell: u16,
        level: u8,
        kind: u8,
        ok: bool,
    }
    struct GroupDisband {
        yourname: String,
        membername: String,
        ok: bool,
    }
    struct GroupFollow {
        name: String,
        ok: bool,
    }
    struct GroupMemberList {
        group_id: u32,
        member_count: u32,
        // scanned member names, '\n'-separated (daemon splits + dedups + self-filters).
        names: String,
        ok: bool,
    }
    struct GroupRoster {
        group_id: u32,
        // full-roster member names, '\n'-separated (solo group = empty).
        names: String,
        ok: bool,
    }
    struct CorpseLoc {
        spawn_id: u32,
        x: f32,
        y: f32,
        z: f32,
        ok: bool,
    }

    // Stage A+7 — variable-length / array opcodes.
    struct Door {
        name: String,
        y: f32,
        x: f32,
        z: f32,
        heading: f32,
        incline: u32,
        size: u32,
        door_id: u8,
        opentype: u8,
        spawnstate: u8,
        invertstate: u8,
        zone_point: u32,
        ok: bool,
    }
    struct GroundSpawn {
        drop_id: u32,
        id_file: String,
        heading: f32,
        y: f32,
        x: f32,
        z: f32,
        bytes_consumed: u32,
        ok: bool,
    }

    // Per-element decode for OP_SendZonePoints. Daemon reads the 4-byte
    // count off the front, then invokes this on each 24-byte
    // zonePointStruct slice.
    struct ZonePoint {
        zone_trigger: u32,
        y: f32,
        x: f32,
        z: f32,
        heading: f32,
        zone_id: u16,
        zone_instance: u16,
        ok: bool,
    }

    // Message opcode payloads. OP_SimpleMessage is fixed 12b; OP_FormattedMessage
    // has a 13b header + variable-length text array (daemon slices the
    // tail off the raw payload); OP_SpecialMesg has two embedded
    // NUL-terminated strings the parser surfaces directly.
    struct SimpleMessage {
        message_format: u32,
        message_color: u32,
        ok: bool,
    }
    // OP_FormattedMessage. `message_format`/`message_color` are the stock
    // Live header (format id + chat colour). The remaining fields are the
    // EQL enrichment and stay zero/empty on live/test: that channel
    // diverges (format id @9, not @5) and multiplexes a spell id, a
    // message-class discriminator, the actor spawn id, and a pre-split
    // NUL-delimited arg list the stock header can't represent. On eql,
    // message_format/message_color mirror format_id/spell_id so the stock
    // MessageShell::formattedMessage symbol still resolves; the eql handler
    // reads the rich fields. See seq-backend-eql/src/formatted_message.rs.
    struct FormattedMessage {
        message_format: u32,
        message_color: u32,
        spell_id: u32,
        msg_type: u8,
        spawn_id: u32,
        format_id: u32,
        args: Vec<String>,
        ok: bool,
    }
    struct SpecialMessage {
        message_color: u32,
        target: u16,
        source: String,
        message: String,
        ok: bool,
    }
    struct ChannelMessage {
        sender: String,
        target: String,
        language: u32,
        chan_num: u32,
        skill_in_language: u32,
        message: String,
        ok: bool,
    }
    struct NewZone {
        short_name: String,
        long_name: String,
        zonefile: String,
        zone_exp_multiplier: f32,
        safe_y: f32,
        safe_x: f32,
        safe_z: f32,
        zone_id: u32,
        ok: bool,
    }

    // OP_PlayerProfile — long, variable-length NetStream walk. Only
    // fields the daemon's downstream consumers actually read are
    // exposed; everything else is parsed (to advance the cursor) but
    // dropped. `bytes_consumed` matches the C++ parser's tally for the
    // length-mismatch debug print.
    struct PlayerProfile {
        ok: bool,
        bytes_consumed: u32,
        checksum: u32,

        // profile.*
        gender: u8,
        race: u32,
        class_: u32,
        class_mask: u32,
        stance: u32,     // EQL active stance ability id (profile @33777); 0 = none
        invocation: u32, // EQL active invocation ability id (profile @33781); 0 = none
        level: u8,
        level1: u8,
        bind0_zone_id: u32,
        bind0_x: f32,
        bind0_y: f32,
        bind0_z: f32,
        bind0_heading: f32,
        deity: u32,
        intoxication: u32,
        points: u32,
        mana: u32,
        cur_hp: u32,
        str_: u32,
        sta: u32,
        cha: u32,
        dex: u32,
        int_: u32,
        agi: u32,
        wis: u32,
        aa_ids: Vec<u32>,
        aa_values: Vec<u32>,
        // Player skill values, index = skill id. Populated by the eql profile
        // walk (parse_player_profile); empty on live/test.
        skills: Vec<u32>,
        disciplines: Vec<u32>,
        recast_timers: Vec<u32>,
        spell_book: Vec<i32>,
        mem_spells: Vec<i32>,
        spell_slot_refresh: Vec<u32>,
        buff_spell_ids: Vec<i32>,
        buff_durations: Vec<i32>,
        platinum: u32,
        gold: u32,
        silver: u32,
        copper: u32,
        platinum_cursor: u32,
        gold_cursor: u32,
        silver_cursor: u32,
        copper_cursor: u32,
        aa_spent: u32,
        aa_assigned: u32,
        aa_unspent: u32,
        endurance: u32,
        exp_aa: u32,

        // charProfileStruct top-level
        name: String,
        last_name: String,
        birthday_time: u32,
        account_create_date: u32,
        last_save_time: u32,
        time_played_min: u32,
        expansions: u32,
        languages: Vec<u8>,
        zone_id: u16,
        zone_instance: u16,
        x: f32,
        y: f32,
        z: f32,
        heading: f32,
        stand_state: u16,
        anon: u16,
        guild_id: u32,
        guild_server_id: u32,
        platinum_inventory: u32,
        gold_inventory: u32,
        silver_inventory: u32,
        copper_inventory: u32,
        platinum_bank: u32,
        gold_bank: u32,
        silver_bank: u32,
        copper_bank: u32,
        platinum_shared: u32,
        career_tribute: u32,
        current_tribute: u32,
        current_rad_crystals: u32,
        career_rad_crystals: u32,
        current_ebon_crystals: u32,
        career_ebon_crystals: u32,
        autosplit: u8,
        ldon_guk_points: u32,
        ldon_mir_points: u32,
        ldon_mmc_points: u32,
        ldon_ruj_points: u32,
        ldon_tak_points: u32,
        ldon_avail_points: u32,
    }

    // Stage A+8 — bitfield-laden / BitStream-packed opcodes.
    struct PlayerSelfPos {
        spawn_id: u16,
        x: f32,
        y: f32,
        z: f32,
        delta_x: f32,
        delta_y: f32,
        delta_z: f32,
        heading: u16,
        delta_heading: i16,
        animation: i16,
        pitch: u16,
        ok: bool,
    }
    struct PlayerSpawnPos {
        spawn_id: u16,
        spawn_id2: u16,
        x: i32,
        y: i32,
        z: i32,
        delta_x: i32,
        delta_y: i32,
        delta_z: i32,
        heading: u16,
        delta_heading: i16,
        animation: i16,
        pitch: u16,
        ok: bool,
    }
    struct NpcMove {
        spawn_id: u16,
        x: i16,
        y: i16,
        z: i16,
        heading: i16,
        delta_x: i16,
        delta_y: i16,
        delta_z: i16,
        delta_heading: i8,
        animation: i16,
        ok: bool,
    }

    // Stateful Session API. cxx cannot express a Rust enum whose variants own
    // different payloads, so a batch uses tagged references into typed payload
    // vectors. C++ switches on `kind`, reads `payload_index` from the matching
    // vector, and can construct its std::variant mechanically.
    enum SessionBackend {
        Live = 0,
        Test = 1,
        Eql = 2,
    }
    enum SessionStream {
        World = 0,
        Zone = 1,
    }
    enum SessionDirection {
        ServerToClient = 0,
        ClientToServer = 1,
    }
    enum SessionFlushReason {
        Shutdown = 0,
        ZoneTransition = 1,
        ReplayEnd = 2,
        Reset = 3,
    }
    enum EventSessionResetReason {
        EnterWorld = 0,
        PlayerProfile = 1,
        ZoneTransition = 2,
        Explicit = 3,
    }
    enum SessionDisposition {
        Decoded = 0,
        Ignored = 1,
        Unhandled = 2,
        Malformed = 3,
        Unmapped = 4,
    }
    enum SessionEventKind {
        SpawnAdded = 0,
        SpawnMoved = 1,
        SpawnRemoved = 2,
        SpawnKilled = 3,
        SpawnHp = 4,
        StatSync = 5,
        SelfPos = 6,
        SpawnAnimation = 7,
        SpawnIllusion = 8,
        GuildsInZone = 9,
        TimeOfDay = 10,
        ZoneChanged = 11,
        PlayerProfile = 12,
        Stance = 13,
        Invocation = 14,
        InspectAnswer = 15,
        GuildRoster = 16,
        ZoneServerInfo = 17,
        ItemSet = 18,
        ItemLearned = 19,
        GuildMotd = 20,
        GuildRankName = 21,
        LoadoutSwap = 22,
        Doors = 23,
        GroundItemRemoved = 24,
        GroundItem = 25,
        Combat = 26,
        SpawnCast = 27,
        Targeted = 28,
        Considered = 29,
        AaTable = 30,
        Exp = 31,
        AaExp = 32,
        Stamina = 33,
        ManaUpdate = 34,
        SkillUpdate = 35,
        LootTransaction = 36,
        LootDrops = 37,
        Money = 38,
        SimpleMessage = 39,
        FormattedMessage = 40,
        SpecialMessage = 41,
        LootMessage = 42,
        Chat = 43,
        BuffList = 44,
        GroupFollow = 45,
        GroupDisband = 46,
        LevelUpdate = 47,
        EnterWorld = 48,
        SessionReset = 49,
        ZoneTransition = 50,
        ZoneEnvironmentChanged = 51,
        SpawnRenamed = 52,
        CorpseLocated = 53,
        ZonePoints = 54,
        PlayerIdentityUpdated = 55,
        PlayerMoved = 56,
        PlayerVitalsUpdated = 57,
        SpawnHealthUpdated = 58,
        PlayerDied = 59,
        SpawnDied = 60,
        SpawnIdentityUpdated = 61,
        PlayerAppearanceUpdated = 62,
        InventorySnapshot = 63,
        InventoryItemUpdated = 64,
        EquipmentSnapshot = 65,
        EquipmentSlotUpdated = 66,
        MoneyBalanceUpdated = 67,
        SkillsSnapshot = 68,
        SkillValueUpdated = 69,
        ExperienceUpdated = 70,
        AlternateAdvancementSnapshot = 71,
        AlternateAdvancementUpdated = 72,
        AlternateAbilityDefined = 73,
    }

    struct SessionEventRef {
        kind: SessionEventKind,
        payload_index: u32,
    }
    struct EventPos {
        x: i32,
        y: i32,
        z: i32,
        heading_deg: u16,
    }
    struct EventVelocity {
        has_x: bool,
        x: i32,
        has_y: bool,
        y: i32,
        has_z: bool,
        z: i32,
    }
    struct EventPoint3 {
        x: f32,
        y: f32,
        z: f32,
    }
    struct EventPlayerIdentity {
        has_spawn_id: bool,
        spawn_id: u32,
        name: String,
        last_name: String,
        race: u32,
        class_: u32,
        deity: u32,
        level: u32,
        class_mask: u32,
    }
    struct EventPlayerMoved {
        has_spawn_id: bool,
        spawn_id: u32,
        pos: EventPos,
    }
    struct EventVitalValue {
        current: i32,
        has_maximum: bool,
        maximum: i32,
    }
    struct EventPlayerVitals {
        has_health: bool,
        health: EventVitalValue,
        has_mana: bool,
        mana: EventVitalValue,
        has_endurance: bool,
        endurance: EventVitalValue,
    }
    struct EventSpawnHealth {
        id: u32,
        current: i32,
        maximum: i32,
    }
    struct EventPlayerDied {
        has_killer_id: bool,
        killer_id: u32,
    }
    struct EventSpawnDied {
        id: u32,
        has_killer_id: bool,
        killer_id: u32,
    }
    struct EventSpawnIdentity {
        id: u32,
        level: u32,
        class_: u32,
        race: u32,
    }
    struct EventPlayerAppearance {
        has_race: bool,
        race: u32,
        has_gender: bool,
        gender: u8,
        has_animation: bool,
        animation: u32,
    }
    struct EventSpawnInfo {
        id: u32,
        name: String,
        last_name: String,
        race: u32,
        class_: u32,
        deity: u32,
        level: u8,
        npc: u8,
        cur_hp: u32,
        has_max_hp: bool,
        max_hp: u32,
        guild_id: u32,
        guild_server_id: u32,
        class_mask: u32,
        has_pos: bool,
        pos: EventPos,
        velocity: EventVelocity,
        has_delta_heading: bool,
        delta_heading: i16,
        has_animation: bool,
        animation: i16,
        has_equipment_models: bool,
        equipment_models: Vec<u32>,
    }
    struct EventProfileInfo {
        name: String,
        last_name: String,
        class_: u32,
        level: u8,
        race: u32,
        deity: u32,
        cur_hp: u32,
        mana: u32,
        aa_ids: Vec<u32>,
        aa_values: Vec<u32>,
        aa_spent: u32,
        aa_assigned: u32,
        aa_unspent: u32,
        aa_experience: u32,
        skills: Vec<u32>,
        class_mask: u32,
        str_: u32,
        sta: u32,
        cha: u32,
        dex: u32,
        int_: u32,
        agi: u32,
        wis: u32,
        platinum: u32,
        gold: u32,
        silver: u32,
        copper: u32,
    }
    struct EventGuildInZone {
        guild_id: u32,
        server_id: u32,
        name: String,
    }
    struct EventGuildRosterMember {
        name: String,
        level: u32,
        class_: u32,
        class_mask: u32,
        rank: u32,
        last_on: u32,
        banker: bool,
        alt: bool,
        full_member: bool,
        public_note: String,
        zone_id: u32,
    }
    struct EventItemTemplate {
        serial: String,
        name: String,
        lore_name: String,
        item_id: u32,
        has_icon: bool,
        icon: u32,
        has_stack_count: bool,
        stack_count: u32,
        has_weight_tenths: bool,
        weight_tenths: u32,
        has_flags: bool,
        flags: u32,
        has_corruption: bool,
        corruption: i32,
        slot_mask: u32,
        container_id: u32,
        container_slot: u16,
        parent_slot: u16,
        stats: Vec<i32>,
        resists: Vec<i32>,
        hp: i32,
        mana: i32,
        endurance: i32,
        ac: i32,
    }
    struct EventDoorInfo {
        id: u32,
        name: String,
        position: EventPoint3,
        heading: f32,
        incline: u32,
        size: u32,
        open_type: u8,
        state: u8,
        invert_state: u8,
        has_zone_point_id: bool,
        zone_point_id: u32,
    }
    struct EventLootItemInfo {
        name: String,
        icon: u32,
        item_id: u32,
    }
    struct EventBuffEntry {
        spell_id: u32,
        remaining_ticks: i32,
        slot: u32,
        caster: String,
    }
    struct EventSpawnMoved {
        id: u32,
        pos: EventPos,
        velocity: EventVelocity,
        has_delta_heading: bool,
        delta_heading: i16,
        has_animation: bool,
        animation: i16,
    }
    struct EventSpawnRenamed {
        has_id: bool,
        id: u32,
        old_name: String,
        new_name: String,
    }
    struct EventSpawnId {
        id: u32,
    }
    struct EventSpawnKilled {
        deceased_id: u32,
        killer_id: u32,
    }
    struct EventSpawnHp {
        id: u32,
        cur: i32,
        max: i32,
    }
    struct EventStatSync {
        spawn_id: u32,
        wide: bool,
        has_hp: bool,
        hp_cur: i32,
        hp_max: i32,
        has_mana: bool,
        mana_cur: i32,
        mana_max: i32,
        has_end: bool,
        end_cur: i32,
        end_max: i32,
    }
    struct EventSelfPos {
        pos: EventPos,
        spawn_id: u32,
        velocity: EventVelocity,
        has_delta_heading: bool,
        delta_heading: i16,
        has_animation: bool,
        animation: i16,
    }
    struct EventSpawnAnimation {
        spawn_id: u32,
        animation: u32,
    }
    struct EventSpawnIllusion {
        spawn_id: u32,
        race: u32,
        gender: u8,
    }
    struct EventGuildsInZone {
        guilds: Vec<EventGuildInZone>,
    }
    struct EventTimeOfDay {
        year: u32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
    }
    struct EventZoneInfo {
        short_name: String,
        long_name: String,
    }
    struct EventSessionReset {
        reason: EventSessionResetReason,
    }
    struct EventZoneTransition {
        character_name: String,
        has_zone_id: bool,
        zone_id: u32,
        has_instance_id: bool,
        instance_id: u32,
        confirmed: bool,
    }
    struct EventZoneEnvironment {
        zone_file: String,
        experience_multiplier: f32,
        safe_x: f32,
        safe_y: f32,
        safe_z: f32,
    }
    struct EventEnterWorld {
        character_name: String,
    }
    struct EventNamed {
        name: String,
    }
    struct EventInspectAnswer {
        spawn_id: u32,
        item_names: Vec<String>,
        bio: String,
    }
    struct EventGuildRoster {
        guild_id: u32,
        members: Vec<EventGuildRosterMember>,
    }
    struct EventZoneServerInfo {
        host: String,
        port: u32,
    }
    struct EventItemSet {
        items: Vec<EventItemTemplate>,
    }
    struct EventItemLearned {
        item: EventItemTemplate,
    }
    struct EventInventorySnapshot {
        items: Vec<EventItemTemplate>,
    }
    struct EventItemLocation {
        container_id: u32,
        container_slot: u16,
        parent_slot: u16,
    }
    struct EventInventoryItemUpdated {
        item: EventItemTemplate,
        has_previous_location: bool,
        previous_location: EventItemLocation,
    }
    struct EventEquipmentSnapshot {
        items: Vec<EventItemTemplate>,
    }
    struct EventEquipmentSlotUpdated {
        slot: u16,
        has_item: bool,
        item: EventItemTemplate,
    }
    struct EventGuildMotdPayload {
        message: String,
        sender: String,
    }
    struct EventGuildRankName {
        guild_id: u32,
        rank_index: u32,
        rank_name: String,
    }
    struct EventLoadoutSwap {
        spawn_id: u32,
        level: u32,
        class_: u32,
        race: u32,
    }
    struct EventDoors {
        doors: Vec<EventDoorInfo>,
    }
    struct EventGroundItemRemoved {
        drop_id: u32,
    }
    struct EventGroundItem {
        id: u32,
        actor_definition: String,
        position: EventPoint3,
        has_heading: bool,
        heading: f32,
    }
    struct EventCorpseLocated {
        id: u32,
        position: EventPoint3,
    }
    struct EventZonePointInfo {
        has_trigger_id: bool,
        trigger_id: u32,
        has_actor_definition: bool,
        actor_definition: String,
        position: EventPoint3,
        heading: f32,
        has_destination_zone_id: bool,
        destination_zone_id: u16,
        has_destination_instance_id: bool,
        destination_instance_id: u16,
    }
    struct EventZonePoints {
        points: Vec<EventZonePointInfo>,
    }
    struct EventCombat {
        source: u32,
        target: u32,
        kind: u32,
        damage: i32,
        spell_id: u32,
    }
    struct EventSpawnCast {
        caster_id: u32,
        spell_id: u32,
        cast_time_ms: u32,
    }
    struct EventAaTable {
        desc_id: u32,
        title_sid: u32,
    }
    struct EventAlternateAbilityDefinition {
        ability_id: u32,
        title_string_id: u32,
    }
    struct EventExp {
        exp: u32,
    }
    struct EventAaExp {
        alt_exp: u32,
        aa_points: u32,
    }
    struct EventAlternateAbilityRank {
        ability_id: u32,
        rank: u32,
    }
    struct EventAlternateAdvancementSnapshot {
        purchased: Vec<EventAlternateAbilityRank>,
        has_spent_points: bool,
        spent_points: u32,
        has_assigned_points: bool,
        assigned_points: u32,
        unspent_points: u32,
        experience: u32,
    }
    struct EventAlternateAdvancementProgress {
        experience: u32,
        unspent_points: u32,
    }
    struct EventStaminaPayload {
        food: u32,
        water: u32,
    }
    struct EventManaUpdate {
        mana: u32,
    }
    struct EventSkillUpdatePayload {
        skill_id: u32,
        value: u32,
    }
    struct EventSkillValue {
        skill_id: u32,
        value: u32,
    }
    struct EventSkillsSnapshot {
        skills: Vec<EventSkillValue>,
    }
    struct EventExperienceProgress {
        experience: u32,
        has_level: bool,
        level: u32,
        has_previous_level: bool,
        previous_level: u32,
    }
    struct EventLootTransactionPayload {
        corpse_id: u32,
        item_id: u32,
        quantity: u32,
        coin_copper: u32,
        from_corpse: bool,
    }
    struct EventLootDropsPayload {
        corpse_id: u32,
        corpse_name: String,
        items: Vec<EventLootItemInfo>,
    }
    struct EventMoney {
        platinum: u32,
        gold: u32,
        silver: u32,
        copper: u32,
    }
    struct EventMoneyBalance {
        platinum: u32,
        gold: u32,
        silver: u32,
        copper: u32,
    }
    struct EventSimpleMessagePayload {
        format_id: u32,
        color: u32,
    }
    struct EventFormattedMessagePayload {
        format_id: u32,
        color: u32,
        args: Vec<String>,
    }
    struct EventSpecialMessagePayload {
        color: u32,
        target: u32,
        source: String,
        message: String,
    }
    struct EventLootMessagePayload {
        color: u32,
        text: String,
        item_id: u32,
        item_name: String,
    }
    struct EventChat {
        channel: u32,
        from: String,
        target: String,
        text: String,
        chat_color: u32,
        channel_name: String,
    }
    struct EventBuffList {
        owner: u32,
        entries: Vec<EventBuffEntry>,
    }
    struct EventGroupFollowPayload {
        name: String,
        level: u32,
    }
    struct EventGroupDisbandPayload {
        yourname: String,
        membername: String,
    }
    struct EventLevelUpdatePayload {
        level: u32,
        level_old: u32,
        exp: u32,
    }
    struct SessionDecodeBatch {
        protocol_generation: u64,
        disposition: SessionDisposition,
        events: Vec<SessionEventRef>,
        player_identity_updated: Vec<EventPlayerIdentity>,
        player_moved: Vec<EventPlayerMoved>,
        player_vitals_updated: Vec<EventPlayerVitals>,
        spawn_health_updated: Vec<EventSpawnHealth>,
        player_died: Vec<EventPlayerDied>,
        spawn_died: Vec<EventSpawnDied>,
        spawn_identity_updated: Vec<EventSpawnIdentity>,
        player_appearance_updated: Vec<EventPlayerAppearance>,
        spawn_added: Vec<EventSpawnInfo>,
        spawn_moved: Vec<EventSpawnMoved>,
        spawn_removed: Vec<EventSpawnId>,
        spawn_renamed: Vec<EventSpawnRenamed>,
        spawn_killed: Vec<EventSpawnKilled>,
        spawn_hp: Vec<EventSpawnHp>,
        stat_sync: Vec<EventStatSync>,
        self_pos: Vec<EventSelfPos>,
        spawn_animation: Vec<EventSpawnAnimation>,
        spawn_illusion: Vec<EventSpawnIllusion>,
        guilds_in_zone: Vec<EventGuildsInZone>,
        time_of_day: Vec<EventTimeOfDay>,
        zone_changed: Vec<EventZoneInfo>,
        session_reset: Vec<EventSessionReset>,
        zone_transition: Vec<EventZoneTransition>,
        zone_environment_changed: Vec<EventZoneEnvironment>,
        player_profile: Vec<EventProfileInfo>,
        named: Vec<EventNamed>,
        inspect_answer: Vec<EventInspectAnswer>,
        guild_roster: Vec<EventGuildRoster>,
        zone_server_info: Vec<EventZoneServerInfo>,
        item_set: Vec<EventItemSet>,
        item_learned: Vec<EventItemLearned>,
        inventory_snapshot: Vec<EventInventorySnapshot>,
        inventory_item_updated: Vec<EventInventoryItemUpdated>,
        equipment_snapshot: Vec<EventEquipmentSnapshot>,
        equipment_slot_updated: Vec<EventEquipmentSlotUpdated>,
        guild_motd: Vec<EventGuildMotdPayload>,
        guild_rank_name: Vec<EventGuildRankName>,
        loadout_swap: Vec<EventLoadoutSwap>,
        doors: Vec<EventDoors>,
        ground_item_removed: Vec<EventGroundItemRemoved>,
        ground_item: Vec<EventGroundItem>,
        corpse_located: Vec<EventCorpseLocated>,
        zone_points: Vec<EventZonePoints>,
        combat: Vec<EventCombat>,
        spawn_cast: Vec<EventSpawnCast>,
        spawn_id: Vec<EventSpawnId>,
        aa_table: Vec<EventAaTable>,
        alternate_ability_defined: Vec<EventAlternateAbilityDefinition>,
        exp: Vec<EventExp>,
        experience_updated: Vec<EventExperienceProgress>,
        aa_exp: Vec<EventAaExp>,
        alternate_advancement_snapshot: Vec<EventAlternateAdvancementSnapshot>,
        alternate_advancement_updated: Vec<EventAlternateAdvancementProgress>,
        stamina: Vec<EventStaminaPayload>,
        mana_update: Vec<EventManaUpdate>,
        skill_update: Vec<EventSkillUpdatePayload>,
        skills_snapshot: Vec<EventSkillsSnapshot>,
        skill_value_updated: Vec<EventSkillValue>,
        loot_transaction: Vec<EventLootTransactionPayload>,
        loot_drops: Vec<EventLootDropsPayload>,
        money: Vec<EventMoney>,
        money_balance_updated: Vec<EventMoneyBalance>,
        simple_message: Vec<EventSimpleMessagePayload>,
        formatted_message: Vec<EventFormattedMessagePayload>,
        special_message: Vec<EventSpecialMessagePayload>,
        loot_message: Vec<EventLootMessagePayload>,
        chat: Vec<EventChat>,
        buff_list: Vec<EventBuffList>,
        group_follow: Vec<EventGroupFollowPayload>,
        group_disband: Vec<EventGroupDisbandPayload>,
        level_update: Vec<EventLevelUpdatePayload>,
        enter_world: Vec<EventEnterWorld>,
        self_stats: Vec<SelfStat>,
        loot_rows: Vec<LootRow>,
    }

    extern "Rust" {
        type SessionProtocolRegistry;
        fn session_protocol_registry_new(
            protocol_dir: &str,
        ) -> Result<Box<SessionProtocolRegistry>>;
        fn reload(
            self: &SessionProtocolRegistry,
            backend: SessionBackend,
            protocol_dir: &str,
        ) -> Result<u64>;
        fn content_hash(self: &SessionProtocolRegistry, backend: SessionBackend) -> String;

        type SessionResource;
        fn session_new(
            registry: &SessionProtocolRegistry,
            backend: SessionBackend,
        ) -> Result<Box<SessionResource>>;
        fn decode(
            self: &mut SessionResource,
            stream: SessionStream,
            opcode_id: u16,
            direction: SessionDirection,
            payload: &[u8],
            timestamp: i64,
        ) -> SessionDecodeBatch;
        fn flush(self: &mut SessionResource, reason: SessionFlushReason) -> SessionDecodeBatch;

        fn decode_mob_update(bytes: &[u8]) -> MobUpdate;
        fn decode_delete_spawn(bytes: &[u8]) -> DeleteSpawn;
        fn decode_spawn(bytes: &[u8]) -> Spawn;
        fn decode_remove_spawn(bytes: &[u8]) -> RemoveSpawn;
        fn decode_hp_update(bytes: &[u8]) -> HpUpdate;
        fn decode_stat_sync(bytes: &[u8]) -> StatSync;

        // eql session identity. Unlike every other entry here this is stateful:
        // eql issues the self ZoneEntry twice per zone and keys the player's
        // stats to the SECOND id, which can land after the stats themselves do.
        // Resolving that needs cross-packet memory, so it lives in the backend
        // where every host inherits it rather than in each host's dispatch.
        // One instance per session/box. Inert on live/test.
        type EqlSelfTracker;
        fn eql_self_tracker_new() -> Box<EqlSelfTracker>;
        fn reset(self: &mut EqlSelfTracker);
        fn self_id(self: &EqlSelfTracker) -> u32;
        fn observe_spawn(
            self: &mut EqlSelfTracker,
            player_name: &str,
            spawn_name: &str,
            spawn_id: u32,
        ) -> u8;
        fn observe_stat_sync(self: &mut EqlSelfTracker, stat: &StatSync) -> SelfStat;
        fn take_pending_vitals(self: &mut EqlSelfTracker) -> SelfStat;
        // Mid-session recovery: with no zone-in witnessed there is no name to
        // match, so the player is invisible until they zone. Feed the id from
        // the client's own outbound position report here — 1 = newly adopted
        // provisionally (synthesise a record for it), 0 = nothing to do.
        fn observe_self_pos(self: &mut EqlSelfTracker, spawn_id: u32) -> u8;
        fn provisional_id(self: &EqlSelfTracker) -> u32;
        // Non-zero when a real (name-matched) adoption has superseded a
        // provisional id: drop whatever was synthesised for it.
        fn take_retired_provisional(self: &mut EqlSelfTracker) -> u32;
        // One acquisition spans two packets (narration, then confirmation), so
        // recording needs cross-packet memory — same reasoning as the self
        // tracker above. One instance per session; inert on live/test. Each
        // method returns the rows that COMPLETED on this event, usually none.
        type EqlLootTracker;
        fn eql_loot_tracker_new() -> Box<EqlLootTracker>;
        fn reset(self: &mut EqlLootTracker);
        fn set_looter(self: &mut EqlLootTracker, looter: &str);
        fn set_zone(self: &mut EqlLootTracker, zone_short: &str) -> Vec<LootRow>;
        fn on_loot_message(
            self: &mut EqlLootTracker,
            color: u32,
            text: &str,
            item_id: u32,
            item_name: &str,
            ts: i64,
        ) -> Vec<LootRow>;
        // Takes the decoded confirmation straight from decode_loot_transaction.
        fn on_loot_transaction(
            self: &mut EqlLootTracker,
            t: &LootTransaction,
            ts: i64,
        ) -> Vec<LootRow>;
        fn on_loot_drop_item(
            self: &mut EqlLootTracker,
            corpse_id: u32,
            corpse_name: &str,
            item_name: &str,
            icon: u32,
            item_id: u32,
            ts: i64,
        ) -> Vec<LootRow>;
        // Emit a narration that never got its confirmation (shutdown, zone-out).
        fn flush(self: &mut EqlLootTracker) -> Vec<LootRow>;
        fn decode_loadout_swap(bytes: &[u8]) -> LoadoutSwap;
        fn decode_money_update(bytes: &[u8]) -> MoneyUpdate;
        fn decode_loot_transaction(bytes: &[u8]) -> LootTransaction;
        fn decode_loot_drops(bytes: &[u8]) -> LootDrops;
        fn decode_buff_list(bytes: &[u8]) -> Vec<BuffListEntry>;
        fn decode_guilds_in_zone_list(bytes: &[u8]) -> Vec<GuildInZoneRow>;
        fn decode_new_guild_in_zone(bytes: &[u8]) -> Vec<GuildInZoneRow>;
        fn decode_guild_motd(bytes: &[u8]) -> GuildMotd;
        fn decode_guild_expanded_info(bytes: &[u8]) -> GuildExpandedInfo;
        fn decode_guild_member_update(bytes: &[u8]) -> GuildMemberUpdateInfo;
        fn decode_guild_roster(bytes: &[u8]) -> Vec<GuildRosterRow>;
        fn decode_self_pos_breadcrumb(bytes: &[u8]) -> Vec<SelfPosPoint>;
        fn decode_ucs_chat(bytes: &[u8]) -> Vec<UcsChatRecord>;
        fn decode_ucs_channels(bytes: &[u8]) -> Vec<String>;
        fn decode_mob_health(bytes: &[u8]) -> MobHealth;
        fn decode_spawn_appearance(bytes: &[u8]) -> SpawnAppearance;
        fn decode_exp_update(bytes: &[u8]) -> ExpUpdate;
        fn decode_level_update(bytes: &[u8]) -> LevelUpdate;
        fn decode_skill_update(bytes: &[u8]) -> SkillUpdate;
        fn decode_mana_change(bytes: &[u8]) -> ManaChange;
        fn decode_stamina(bytes: &[u8]) -> Stamina;
        fn decode_end_update(bytes: &[u8]) -> EndUpdate;
        fn decode_consider(bytes: &[u8]) -> Consider;
        fn decode_spawn_rename(bytes: &[u8]) -> SpawnRename;
        fn decode_client_target(bytes: &[u8]) -> ClientTarget;
        fn decode_death(bytes: &[u8]) -> Death;
        fn decode_click_object(bytes: &[u8]) -> ClickObject;
        fn decode_illusion(bytes: &[u8]) -> Illusion;
        fn decode_buff(bytes: &[u8]) -> Buff;
        fn decode_action2(bytes: &[u8]) -> Action2;
        // Stage A+6
        fn decode_wear_change(bytes: &[u8]) -> WearChange;
        fn decode_zone_change(bytes: &[u8]) -> ZoneChange;
        fn decode_dz_info(bytes: &[u8]) -> DzInfo;
        fn decode_dz_switch_info(bytes: &[u8]) -> DzSwitch;
        fn decode_start_cast(bytes: &[u8]) -> StartCast;
        fn decode_begin_cast(bytes: &[u8]) -> BeginCast;
        fn decode_activate_ability(bytes: &[u8]) -> ActivateAbility;
        fn decode_aa_table_entry(bytes: &[u8]) -> AaTableEntry;
        fn decode_action(bytes: &[u8]) -> Action;
        fn decode_action_alt(bytes: &[u8]) -> Action;
        fn decode_group_disband(bytes: &[u8]) -> GroupDisband;
        fn decode_group_follow(bytes: &[u8]) -> GroupFollow;
        fn decode_group_member_list(bytes: &[u8]) -> GroupMemberList;
        fn decode_group_roster(bytes: &[u8]) -> GroupRoster;
        fn decode_corpse_loc(bytes: &[u8]) -> CorpseLoc;
        // Stage A+7
        fn decode_door(bytes: &[u8]) -> Door;
        fn decode_ground_spawn(bytes: &[u8]) -> GroundSpawn;
        fn decode_zone_point(bytes: &[u8]) -> ZonePoint;
        // Message opcodes
        fn decode_simple_message(bytes: &[u8]) -> SimpleMessage;
        fn decode_formatted_message(bytes: &[u8]) -> FormattedMessage;
        fn decode_special_message(bytes: &[u8]) -> SpecialMessage;
        fn decode_loot_message(bytes: &[u8]) -> LootMessage;
        fn decode_channel_message(bytes: &[u8]) -> ChannelMessage;
        fn decode_new_zone(bytes: &[u8]) -> NewZone;
        fn decode_player_profile(bytes: &[u8]) -> PlayerProfile;
        // Stage A+8
        fn decode_player_self_pos(bytes: &[u8]) -> PlayerSelfPos;
        fn decode_player_spawn_pos(bytes: &[u8]) -> PlayerSpawnPos;
        fn decode_npc_move_update(bytes: &[u8]) -> NpcMove;

        /// Backend-sourced payload size overrides (see `StructSize`). Empty on
        /// live/test; eql returns the payloads whose wire size diverges from
        /// Live's compiled `everquest.h` struct.
        fn struct_size_overrides() -> Vec<StructSize>;

        /// Per-row byte stride of an `OP_SpawnDoor` array payload for the
        /// linked backend (136 on live/test = `sizeof(doorStruct)`; 132 on
        /// eql). The daemon's `SpawnShell::newDoorSpawns` iterates with this
        /// instead of the compiled Live `sizeof`, which would mis-stride a
        /// diverged backend's rows.
        fn door_stride() -> usize;
    }
}

pub struct SessionProtocolRegistry(std::sync::Arc<seq_protocol_data::ProtocolRegistry>);

fn session_protocol_registry_new(
    protocol_dir: &str,
) -> Result<Box<SessionProtocolRegistry>, String> {
    let registry = if protocol_dir.is_empty() {
        seq_protocol_data::ProtocolRegistry::embedded().map_err(|error| error.to_string())?
    } else {
        seq_protocol_data::ProtocolRegistry::from_directory(protocol_dir)
            .map_err(|error| error.to_string())?
    };
    Ok(Box::new(SessionProtocolRegistry(std::sync::Arc::new(
        registry,
    ))))
}

impl SessionProtocolRegistry {
    fn reload(&self, backend: ffi::SessionBackend, protocol_dir: &str) -> Result<u64, String> {
        self.0
            .reload_backend_from_directory(protocol_dir, protocol_backend(backend))
            .map(|generation| generation.0)
            .map_err(|error| error.to_string())
    }

    fn content_hash(&self, backend: ffi::SessionBackend) -> String {
        self.0
            .snapshot(protocol_backend(backend))
            .content_hash()
            .to_hex()
    }
}

pub struct SessionResource {
    session: seq_session::Session,
    registry: std::sync::Arc<seq_protocol_data::ProtocolRegistry>,
    backend: seq_protocol_data::BackendId,
}

fn session_new(
    registry: &SessionProtocolRegistry,
    backend: ffi::SessionBackend,
) -> Result<Box<SessionResource>, String> {
    let backend = protocol_backend(backend);
    if backend != linked_backend() {
        return Err(format!(
            "seq-bridge was built for {}, not {backend}",
            linked_backend()
        ));
    }
    let protocol_registry = std::sync::Arc::clone(&registry.0);
    let session = seq_session::Session::new(seq_session::SessionConfig {
        backend,
        protocol_registry: std::sync::Arc::clone(&protocol_registry),
    });
    Ok(Box::new(SessionResource {
        session,
        registry: protocol_registry,
        backend,
    }))
}

impl SessionResource {
    fn decode(
        &mut self,
        stream: ffi::SessionStream,
        opcode_id: u16,
        direction: ffi::SessionDirection,
        payload: &[u8],
        timestamp: i64,
    ) -> ffi::SessionDecodeBatch {
        let decoded = self.session.decode_at(
            protocol_stream(stream),
            seq_protocol_data::OpcodeId(opcode_id),
            session_direction(direction),
            payload,
            timestamp,
        );
        let mut batch = translate_events(
            decoded.protocol_generation.0,
            session_disposition(decoded.disposition),
            decoded.events,
        );
        self.drain_correlations(&mut batch);
        batch
    }

    fn flush(&mut self, reason: ffi::SessionFlushReason) -> ffi::SessionDecodeBatch {
        let events = self.session.flush(session_flush_reason(reason));
        let generation = self.registry.snapshot(self.backend).generation().0;
        let disposition = if events.is_empty() {
            ffi::SessionDisposition::Ignored
        } else {
            ffi::SessionDisposition::Decoded
        };
        let mut batch = translate_events(generation, disposition, events);
        self.drain_correlations(&mut batch);
        batch
    }

    fn drain_correlations(&mut self, _batch: &mut ffi::SessionDecodeBatch) {
        #[cfg(feature = "backend-eql")]
        {
            _batch.self_stats = self
                .session
                .take_self_stats()
                .into_iter()
                .map(self_stat_to_ffi)
                .collect();
            _batch.loot_rows = loot_rows_to_ffi(self.session.take_loot_rows());
        }
    }
}

fn linked_backend() -> seq_protocol_data::BackendId {
    #[cfg(feature = "backend-live")]
    return seq_protocol_data::BackendId::Live;
    #[cfg(feature = "backend-test")]
    return seq_protocol_data::BackendId::Test;
    #[cfg(feature = "backend-eql")]
    return seq_protocol_data::BackendId::Eql;
}

fn protocol_backend(backend: ffi::SessionBackend) -> seq_protocol_data::BackendId {
    match backend {
        ffi::SessionBackend::Live => seq_protocol_data::BackendId::Live,
        ffi::SessionBackend::Test => seq_protocol_data::BackendId::Test,
        ffi::SessionBackend::Eql => seq_protocol_data::BackendId::Eql,
        _ => unreachable!("cxx SessionBackend has no unknown values"),
    }
}

fn protocol_stream(stream: ffi::SessionStream) -> seq_protocol_data::StreamKind {
    match stream {
        ffi::SessionStream::World => seq_protocol_data::StreamKind::World,
        ffi::SessionStream::Zone => seq_protocol_data::StreamKind::Zone,
        _ => unreachable!("cxx SessionStream has no unknown values"),
    }
}

fn session_direction(direction: ffi::SessionDirection) -> seq_events::Dir {
    match direction {
        ffi::SessionDirection::ServerToClient => seq_events::Dir::ServerToClient,
        ffi::SessionDirection::ClientToServer => seq_events::Dir::ClientToServer,
        _ => unreachable!("cxx SessionDirection has no unknown values"),
    }
}

fn session_flush_reason(reason: ffi::SessionFlushReason) -> seq_session::FlushReason {
    match reason {
        ffi::SessionFlushReason::Shutdown => seq_session::FlushReason::Shutdown,
        ffi::SessionFlushReason::ZoneTransition => seq_session::FlushReason::ZoneTransition,
        ffi::SessionFlushReason::ReplayEnd => seq_session::FlushReason::ReplayEnd,
        ffi::SessionFlushReason::Reset => seq_session::FlushReason::Reset,
        _ => unreachable!("cxx SessionFlushReason has no unknown values"),
    }
}

fn session_disposition(disposition: seq_session::DecodeDisposition) -> ffi::SessionDisposition {
    match disposition {
        seq_session::DecodeDisposition::Decoded => ffi::SessionDisposition::Decoded,
        seq_session::DecodeDisposition::Ignored => ffi::SessionDisposition::Ignored,
        seq_session::DecodeDisposition::Unhandled => ffi::SessionDisposition::Unhandled,
        seq_session::DecodeDisposition::Malformed => ffi::SessionDisposition::Malformed,
        seq_session::DecodeDisposition::Unmapped => ffi::SessionDisposition::Unmapped,
    }
}

fn event_session_reset_reason(
    reason: seq_events::SessionResetReason,
) -> ffi::EventSessionResetReason {
    match reason {
        seq_events::SessionResetReason::EnterWorld => ffi::EventSessionResetReason::EnterWorld,
        seq_events::SessionResetReason::PlayerProfile => {
            ffi::EventSessionResetReason::PlayerProfile
        }
        seq_events::SessionResetReason::ZoneTransition => {
            ffi::EventSessionResetReason::ZoneTransition
        }
        seq_events::SessionResetReason::Explicit => ffi::EventSessionResetReason::Explicit,
    }
}

fn event_pos(pos: seq_events::Pos) -> ffi::EventPos {
    ffi::EventPos {
        x: pos.x,
        y: pos.y,
        z: pos.z,
        heading_deg: pos.heading_deg,
    }
}

fn event_vital(value: Option<seq_events::VitalValue>) -> ffi::EventVitalValue {
    let value = value.unwrap_or(seq_events::VitalValue {
        current: 0,
        maximum: None,
    });
    ffi::EventVitalValue {
        current: value.current,
        has_maximum: value.maximum.is_some(),
        maximum: value.maximum.unwrap_or_default(),
    }
}

fn event_player_identity(identity: seq_events::PlayerIdentity) -> ffi::EventPlayerIdentity {
    ffi::EventPlayerIdentity {
        has_spawn_id: identity.spawn_id.is_some(),
        spawn_id: identity.spawn_id.unwrap_or_default(),
        name: identity.name,
        last_name: identity.last_name,
        race: identity.race,
        class_: identity.class_,
        deity: identity.deity,
        level: identity.level,
        class_mask: identity.class_mask,
    }
}

fn event_point3(point: seq_events::Point3) -> ffi::EventPoint3 {
    ffi::EventPoint3 {
        x: point.x,
        y: point.y,
        z: point.z,
    }
}

fn event_velocity(velocity: seq_events::Velocity) -> ffi::EventVelocity {
    ffi::EventVelocity {
        has_x: velocity.x.is_some(),
        x: velocity.x.unwrap_or_default(),
        has_y: velocity.y.is_some(),
        y: velocity.y.unwrap_or_default(),
        has_z: velocity.z.is_some(),
        z: velocity.z.unwrap_or_default(),
    }
}

fn event_spawn_info(spawn: seq_events::SpawnInfo) -> ffi::EventSpawnInfo {
    let (has_max_hp, max_hp) = match spawn.max_hp {
        Some(max_hp) => (true, max_hp),
        None => (false, 0),
    };
    let (has_pos, pos) = match spawn.pos {
        Some(pos) => (true, event_pos(pos)),
        None => (
            false,
            ffi::EventPos {
                x: 0,
                y: 0,
                z: 0,
                heading_deg: 0,
            },
        ),
    };
    ffi::EventSpawnInfo {
        id: spawn.id,
        name: spawn.name,
        last_name: spawn.last_name,
        race: spawn.race,
        class_: spawn.class_,
        deity: spawn.deity,
        level: spawn.level,
        npc: spawn.npc,
        cur_hp: spawn.cur_hp,
        has_max_hp,
        max_hp,
        guild_id: spawn.guild_id,
        guild_server_id: spawn.guild_server_id,
        class_mask: spawn.class_mask,
        has_pos,
        pos,
        velocity: event_velocity(spawn.velocity),
        has_delta_heading: spawn.delta_heading.is_some(),
        delta_heading: spawn.delta_heading.unwrap_or_default(),
        has_animation: spawn.animation.is_some(),
        animation: spawn.animation.unwrap_or_default(),
        has_equipment_models: spawn.equipment_models.is_some(),
        equipment_models: spawn.equipment_models.map_or_else(Vec::new, <[_; 9]>::into),
    }
}

fn event_profile(profile: seq_events::ProfileInfo) -> ffi::EventProfileInfo {
    ffi::EventProfileInfo {
        name: profile.name,
        last_name: profile.last_name,
        class_: profile.class_,
        level: profile.level,
        race: profile.race,
        deity: profile.deity,
        cur_hp: profile.cur_hp,
        mana: profile.mana,
        aa_ids: profile.aa_ids,
        aa_values: profile.aa_values,
        aa_spent: profile.aa_spent,
        aa_assigned: profile.aa_assigned,
        aa_unspent: profile.aa_unspent,
        aa_experience: profile.aa_experience,
        skills: profile.skills,
        class_mask: profile.class_mask,
        str_: profile.str_,
        sta: profile.sta,
        cha: profile.cha,
        dex: profile.dex,
        int_: profile.int_,
        agi: profile.agi,
        wis: profile.wis,
        platinum: profile.platinum,
        gold: profile.gold,
        silver: profile.silver,
        copper: profile.copper,
    }
}

fn event_guild(guild: seq_events::GuildInZone) -> ffi::EventGuildInZone {
    ffi::EventGuildInZone {
        guild_id: guild.guild_id,
        server_id: guild.server_id,
        name: guild.name,
    }
}

fn event_roster_member(member: seq_events::GuildRosterMember) -> ffi::EventGuildRosterMember {
    ffi::EventGuildRosterMember {
        name: member.name,
        level: member.level,
        class_: member.class,
        class_mask: member.class_mask,
        rank: member.rank,
        last_on: member.last_on,
        banker: member.banker,
        alt: member.alt,
        full_member: member.full_member,
        public_note: member.public_note,
        zone_id: member.zone_id,
    }
}

fn event_item(item: seq_events::ItemTemplate) -> ffi::EventItemTemplate {
    ffi::EventItemTemplate {
        serial: item.serial,
        name: item.name,
        lore_name: item.lore_name,
        item_id: item.item_id,
        has_icon: item.icon.is_some(),
        icon: item.icon.unwrap_or_default(),
        has_stack_count: item.stack_count.is_some(),
        stack_count: item.stack_count.unwrap_or_default(),
        has_weight_tenths: item.weight_tenths.is_some(),
        weight_tenths: item.weight_tenths.unwrap_or_default(),
        has_flags: item.flags.is_some(),
        flags: item.flags.unwrap_or_default(),
        has_corruption: item.corruption.is_some(),
        corruption: item.corruption.unwrap_or_default(),
        slot_mask: item.slot_mask,
        container_id: item.container_id,
        container_slot: item.container_slot,
        parent_slot: item.parent_slot,
        stats: item.stats,
        resists: item.resists,
        hp: item.hp,
        mana: item.mana,
        endurance: item.endurance,
        ac: item.ac,
    }
}

fn empty_event_item() -> ffi::EventItemTemplate {
    event_item(seq_events::ItemTemplate {
        serial: String::new(),
        name: String::new(),
        lore_name: String::new(),
        item_id: 0,
        icon: None,
        stack_count: None,
        weight_tenths: None,
        flags: None,
        corruption: None,
        slot_mask: 0,
        container_id: 0,
        container_slot: 0,
        parent_slot: 0,
        stats: Vec::new(),
        resists: Vec::new(),
        hp: 0,
        mana: 0,
        endurance: 0,
        ac: 0,
    })
}

fn event_door(door: seq_events::DoorInfo) -> ffi::EventDoorInfo {
    ffi::EventDoorInfo {
        id: door.id,
        name: door.name,
        position: event_point3(door.position),
        heading: door.heading,
        incline: door.incline,
        size: door.size,
        open_type: door.open_type,
        state: door.state,
        invert_state: door.invert_state,
        has_zone_point_id: door.zone_point_id.is_some(),
        zone_point_id: door.zone_point_id.unwrap_or_default(),
    }
}

fn event_zone_point(point: seq_events::ZonePointInfo) -> ffi::EventZonePointInfo {
    ffi::EventZonePointInfo {
        has_trigger_id: point.trigger_id.is_some(),
        trigger_id: point.trigger_id.unwrap_or_default(),
        has_actor_definition: point.actor_definition.is_some(),
        actor_definition: point.actor_definition.unwrap_or_default(),
        position: event_point3(point.position),
        heading: point.heading,
        has_destination_zone_id: point.destination_zone_id.is_some(),
        destination_zone_id: point.destination_zone_id.unwrap_or_default(),
        has_destination_instance_id: point.destination_instance_id.is_some(),
        destination_instance_id: point.destination_instance_id.unwrap_or_default(),
    }
}

fn event_loot_item(item: seq_events::LootItemInfo) -> ffi::EventLootItemInfo {
    ffi::EventLootItemInfo {
        name: item.name,
        icon: item.icon,
        item_id: item.item_id,
    }
}

fn event_buff(buff: seq_events::BuffEntry) -> ffi::EventBuffEntry {
    ffi::EventBuffEntry {
        spell_id: buff.spell_id,
        remaining_ticks: buff.remaining_ticks,
        slot: buff.slot,
        caster: buff.caster,
    }
}

fn push_ref(
    batch: &mut ffi::SessionDecodeBatch,
    kind: ffi::SessionEventKind,
    payload_index: usize,
) {
    batch.events.push(ffi::SessionEventRef {
        kind,
        payload_index: payload_index
            .try_into()
            .expect("a decode batch cannot contain more than u32::MAX events"),
    });
}

fn translate_events(
    protocol_generation: u64,
    disposition: ffi::SessionDisposition,
    events: Vec<seq_events::Event>,
) -> ffi::SessionDecodeBatch {
    let mut batch = empty_session_batch(protocol_generation, disposition);
    for event in events {
        translate_event(&mut batch, event);
    }
    batch
}

#[allow(clippy::too_many_lines)]
fn translate_event(batch: &mut ffi::SessionDecodeBatch, event: seq_events::Event) {
    use seq_events::Event;
    match event {
        Event::SessionReset { reason } => {
            let index = batch.session_reset.len();
            batch.session_reset.push(ffi::EventSessionReset {
                reason: event_session_reset_reason(reason),
            });
            push_ref(batch, ffi::SessionEventKind::SessionReset, index);
        }
        Event::PlayerIdentityUpdated(identity) => {
            let index = batch.player_identity_updated.len();
            batch
                .player_identity_updated
                .push(event_player_identity(identity));
            push_ref(batch, ffi::SessionEventKind::PlayerIdentityUpdated, index);
        }
        Event::PlayerMoved { spawn_id, pos } => {
            let index = batch.player_moved.len();
            batch.player_moved.push(ffi::EventPlayerMoved {
                has_spawn_id: spawn_id.is_some(),
                spawn_id: spawn_id.unwrap_or_default(),
                pos: event_pos(pos),
            });
            push_ref(batch, ffi::SessionEventKind::PlayerMoved, index);
        }
        Event::PlayerVitalsUpdated(vitals) => {
            let index = batch.player_vitals_updated.len();
            batch.player_vitals_updated.push(ffi::EventPlayerVitals {
                has_health: vitals.health.is_some(),
                health: event_vital(vitals.health),
                has_mana: vitals.mana.is_some(),
                mana: event_vital(vitals.mana),
                has_endurance: vitals.endurance.is_some(),
                endurance: event_vital(vitals.endurance),
            });
            push_ref(batch, ffi::SessionEventKind::PlayerVitalsUpdated, index);
        }
        Event::SpawnHealthUpdated {
            id,
            current,
            maximum,
        } => {
            let index = batch.spawn_health_updated.len();
            batch.spawn_health_updated.push(ffi::EventSpawnHealth {
                id,
                current,
                maximum,
            });
            push_ref(batch, ffi::SessionEventKind::SpawnHealthUpdated, index);
        }
        Event::PlayerDied { killer_id } => {
            let index = batch.player_died.len();
            batch.player_died.push(ffi::EventPlayerDied {
                has_killer_id: killer_id.is_some(),
                killer_id: killer_id.unwrap_or_default(),
            });
            push_ref(batch, ffi::SessionEventKind::PlayerDied, index);
        }
        Event::SpawnDied { id, killer_id } => {
            let index = batch.spawn_died.len();
            batch.spawn_died.push(ffi::EventSpawnDied {
                id,
                has_killer_id: killer_id.is_some(),
                killer_id: killer_id.unwrap_or_default(),
            });
            push_ref(batch, ffi::SessionEventKind::SpawnDied, index);
        }
        Event::SpawnIdentityUpdated {
            id,
            level,
            class_,
            race,
        } => {
            let index = batch.spawn_identity_updated.len();
            batch.spawn_identity_updated.push(ffi::EventSpawnIdentity {
                id,
                level,
                class_,
                race,
            });
            push_ref(batch, ffi::SessionEventKind::SpawnIdentityUpdated, index);
        }
        Event::PlayerAppearanceUpdated(appearance) => {
            let index = batch.player_appearance_updated.len();
            batch
                .player_appearance_updated
                .push(ffi::EventPlayerAppearance {
                    has_race: appearance.race.is_some(),
                    race: appearance.race.unwrap_or_default(),
                    has_gender: appearance.gender.is_some(),
                    gender: appearance.gender.unwrap_or_default(),
                    has_animation: appearance.animation.is_some(),
                    animation: appearance.animation.unwrap_or_default(),
                });
            push_ref(batch, ffi::SessionEventKind::PlayerAppearanceUpdated, index);
        }
        Event::SpawnAdded(spawn) => {
            let index = batch.spawn_added.len();
            batch.spawn_added.push(event_spawn_info(spawn));
            push_ref(batch, ffi::SessionEventKind::SpawnAdded, index);
        }
        Event::SpawnMoved {
            id,
            pos,
            velocity,
            delta_heading,
            animation,
        } => {
            let index = batch.spawn_moved.len();
            batch.spawn_moved.push(ffi::EventSpawnMoved {
                id,
                pos: event_pos(pos),
                velocity: event_velocity(velocity),
                has_delta_heading: delta_heading.is_some(),
                delta_heading: delta_heading.unwrap_or_default(),
                has_animation: animation.is_some(),
                animation: animation.unwrap_or_default(),
            });
            push_ref(batch, ffi::SessionEventKind::SpawnMoved, index);
        }
        Event::SpawnRemoved { id } => {
            let index = batch.spawn_removed.len();
            batch.spawn_removed.push(ffi::EventSpawnId { id });
            push_ref(batch, ffi::SessionEventKind::SpawnRemoved, index);
        }
        Event::SpawnRenamed {
            id,
            old_name,
            new_name,
        } => {
            let index = batch.spawn_renamed.len();
            batch.spawn_renamed.push(ffi::EventSpawnRenamed {
                has_id: id.is_some(),
                id: id.unwrap_or_default(),
                old_name,
                new_name,
            });
            push_ref(batch, ffi::SessionEventKind::SpawnRenamed, index);
        }
        Event::SpawnKilled {
            deceased_id,
            killer_id,
        } => {
            let index = batch.spawn_killed.len();
            batch.spawn_killed.push(ffi::EventSpawnKilled {
                deceased_id,
                killer_id,
            });
            push_ref(batch, ffi::SessionEventKind::SpawnKilled, index);
        }
        Event::SpawnHp { id, cur, max } => {
            let index = batch.spawn_hp.len();
            batch.spawn_hp.push(ffi::EventSpawnHp { id, cur, max });
            push_ref(batch, ffi::SessionEventKind::SpawnHp, index);
        }
        Event::StatSync {
            spawn_id,
            wide,
            has_hp,
            hp_cur,
            hp_max,
            has_mana,
            mana_cur,
            mana_max,
            has_end,
            end_cur,
            end_max,
        } => {
            let index = batch.stat_sync.len();
            batch.stat_sync.push(ffi::EventStatSync {
                spawn_id,
                wide,
                has_hp,
                hp_cur,
                hp_max,
                has_mana,
                mana_cur,
                mana_max,
                has_end,
                end_cur,
                end_max,
            });
            push_ref(batch, ffi::SessionEventKind::StatSync, index);
        }
        Event::SelfPos {
            pos,
            spawn_id,
            velocity,
            delta_heading,
            animation,
        } => {
            let index = batch.self_pos.len();
            batch.self_pos.push(ffi::EventSelfPos {
                pos: event_pos(pos),
                spawn_id,
                velocity: event_velocity(velocity),
                has_delta_heading: delta_heading.is_some(),
                delta_heading: delta_heading.unwrap_or_default(),
                has_animation: animation.is_some(),
                animation: animation.unwrap_or_default(),
            });
            push_ref(batch, ffi::SessionEventKind::SelfPos, index);
        }
        Event::SpawnAnimation {
            spawn_id,
            animation,
        } => {
            let index = batch.spawn_animation.len();
            batch.spawn_animation.push(ffi::EventSpawnAnimation {
                spawn_id,
                animation,
            });
            push_ref(batch, ffi::SessionEventKind::SpawnAnimation, index);
        }
        Event::SpawnIllusion {
            spawn_id,
            race,
            gender,
        } => {
            let index = batch.spawn_illusion.len();
            batch.spawn_illusion.push(ffi::EventSpawnIllusion {
                spawn_id,
                race,
                gender,
            });
            push_ref(batch, ffi::SessionEventKind::SpawnIllusion, index);
        }
        Event::GuildsInZone { guilds } => {
            let index = batch.guilds_in_zone.len();
            batch.guilds_in_zone.push(ffi::EventGuildsInZone {
                guilds: guilds.into_iter().map(event_guild).collect(),
            });
            push_ref(batch, ffi::SessionEventKind::GuildsInZone, index);
        }
        Event::TimeOfDay {
            year,
            month,
            day,
            hour,
            minute,
        } => {
            let index = batch.time_of_day.len();
            batch.time_of_day.push(ffi::EventTimeOfDay {
                year,
                month,
                day,
                hour,
                minute,
            });
            push_ref(batch, ffi::SessionEventKind::TimeOfDay, index);
        }
        Event::ZoneTransition {
            character_name,
            zone_id,
            instance_id,
            confirmed,
        } => {
            let index = batch.zone_transition.len();
            batch.zone_transition.push(ffi::EventZoneTransition {
                character_name,
                has_zone_id: zone_id.is_some(),
                zone_id: zone_id.unwrap_or_default(),
                has_instance_id: instance_id.is_some(),
                instance_id: instance_id.unwrap_or_default(),
                confirmed,
            });
            push_ref(batch, ffi::SessionEventKind::ZoneTransition, index);
        }
        Event::ZoneChanged(zone) => {
            let index = batch.zone_changed.len();
            batch.zone_changed.push(ffi::EventZoneInfo {
                short_name: zone.short_name,
                long_name: zone.long_name,
            });
            push_ref(batch, ffi::SessionEventKind::ZoneChanged, index);
        }
        Event::ZoneEnvironmentChanged(environment) => {
            let index = batch.zone_environment_changed.len();
            batch
                .zone_environment_changed
                .push(ffi::EventZoneEnvironment {
                    zone_file: environment.zone_file,
                    experience_multiplier: environment.experience_multiplier,
                    safe_x: environment.safe_x,
                    safe_y: environment.safe_y,
                    safe_z: environment.safe_z,
                });
            push_ref(batch, ffi::SessionEventKind::ZoneEnvironmentChanged, index);
        }
        Event::PlayerProfile(profile) => {
            let index = batch.player_profile.len();
            batch.player_profile.push(event_profile(profile));
            push_ref(batch, ffi::SessionEventKind::PlayerProfile, index);
        }
        Event::Stance { name } => {
            let index = batch.named.len();
            batch.named.push(ffi::EventNamed { name });
            push_ref(batch, ffi::SessionEventKind::Stance, index);
        }
        Event::Invocation { name } => {
            let index = batch.named.len();
            batch.named.push(ffi::EventNamed { name });
            push_ref(batch, ffi::SessionEventKind::Invocation, index);
        }
        Event::InspectAnswer {
            spawn_id,
            item_names,
            bio,
        } => {
            let index = batch.inspect_answer.len();
            batch.inspect_answer.push(ffi::EventInspectAnswer {
                spawn_id,
                item_names,
                bio,
            });
            push_ref(batch, ffi::SessionEventKind::InspectAnswer, index);
        }
        Event::GuildRoster { guild_id, members } => {
            let index = batch.guild_roster.len();
            batch.guild_roster.push(ffi::EventGuildRoster {
                guild_id,
                members: members.into_iter().map(event_roster_member).collect(),
            });
            push_ref(batch, ffi::SessionEventKind::GuildRoster, index);
        }
        Event::ZoneServerInfo { host, port } => {
            let index = batch.zone_server_info.len();
            batch
                .zone_server_info
                .push(ffi::EventZoneServerInfo { host, port });
            push_ref(batch, ffi::SessionEventKind::ZoneServerInfo, index);
        }
        Event::ItemSet { items } => {
            let index = batch.item_set.len();
            batch.item_set.push(ffi::EventItemSet {
                items: items.into_iter().map(event_item).collect(),
            });
            push_ref(batch, ffi::SessionEventKind::ItemSet, index);
        }
        Event::ItemLearned { item } => {
            let index = batch.item_learned.len();
            batch.item_learned.push(ffi::EventItemLearned {
                item: event_item(item),
            });
            push_ref(batch, ffi::SessionEventKind::ItemLearned, index);
        }
        Event::InventorySnapshot { items } => {
            let index = batch.inventory_snapshot.len();
            batch.inventory_snapshot.push(ffi::EventInventorySnapshot {
                items: items.into_iter().map(event_item).collect(),
            });
            push_ref(batch, ffi::SessionEventKind::InventorySnapshot, index);
        }
        Event::InventoryItemUpdated {
            item,
            previous_location,
        } => {
            let index = batch.inventory_item_updated.len();
            let has_previous_location = previous_location.is_some();
            let previous_location = previous_location.unwrap_or(seq_events::ItemLocation {
                container_id: 0,
                container_slot: 0,
                parent_slot: 0,
            });
            batch
                .inventory_item_updated
                .push(ffi::EventInventoryItemUpdated {
                    item: event_item(item),
                    has_previous_location,
                    previous_location: ffi::EventItemLocation {
                        container_id: previous_location.container_id,
                        container_slot: previous_location.container_slot,
                        parent_slot: previous_location.parent_slot,
                    },
                });
            push_ref(batch, ffi::SessionEventKind::InventoryItemUpdated, index);
        }
        Event::EquipmentSnapshot { items } => {
            let index = batch.equipment_snapshot.len();
            batch.equipment_snapshot.push(ffi::EventEquipmentSnapshot {
                items: items.into_iter().map(event_item).collect(),
            });
            push_ref(batch, ffi::SessionEventKind::EquipmentSnapshot, index);
        }
        Event::EquipmentSlotUpdated { slot, item } => {
            let index = batch.equipment_slot_updated.len();
            let has_item = item.is_some();
            batch
                .equipment_slot_updated
                .push(ffi::EventEquipmentSlotUpdated {
                    slot,
                    has_item,
                    item: item.map_or_else(empty_event_item, event_item),
                });
            push_ref(batch, ffi::SessionEventKind::EquipmentSlotUpdated, index);
        }
        Event::GuildMotd { message, sender } => {
            let index = batch.guild_motd.len();
            batch
                .guild_motd
                .push(ffi::EventGuildMotdPayload { message, sender });
            push_ref(batch, ffi::SessionEventKind::GuildMotd, index);
        }
        Event::GuildRankName {
            guild_id,
            rank_index,
            rank_name,
        } => {
            let index = batch.guild_rank_name.len();
            batch.guild_rank_name.push(ffi::EventGuildRankName {
                guild_id,
                rank_index,
                rank_name,
            });
            push_ref(batch, ffi::SessionEventKind::GuildRankName, index);
        }
        Event::LoadoutSwap {
            spawn_id,
            level,
            class,
            race,
        } => {
            let index = batch.loadout_swap.len();
            batch.loadout_swap.push(ffi::EventLoadoutSwap {
                spawn_id,
                level,
                class_: class,
                race,
            });
            push_ref(batch, ffi::SessionEventKind::LoadoutSwap, index);
        }
        Event::Doors(doors) => {
            let index = batch.doors.len();
            batch.doors.push(ffi::EventDoors {
                doors: doors.into_iter().map(event_door).collect(),
            });
            push_ref(batch, ffi::SessionEventKind::Doors, index);
        }
        Event::GroundItemRemoved { drop_id } => {
            let index = batch.ground_item_removed.len();
            batch
                .ground_item_removed
                .push(ffi::EventGroundItemRemoved { drop_id });
            push_ref(batch, ffi::SessionEventKind::GroundItemRemoved, index);
        }
        Event::GroundItem(item) => {
            let index = batch.ground_item.len();
            batch.ground_item.push(ffi::EventGroundItem {
                id: item.id,
                actor_definition: item.actor_definition,
                position: event_point3(item.position),
                has_heading: item.heading.is_some(),
                heading: item.heading.unwrap_or_default(),
            });
            push_ref(batch, ffi::SessionEventKind::GroundItem, index);
        }
        Event::CorpseLocated { id, position } => {
            let index = batch.corpse_located.len();
            batch.corpse_located.push(ffi::EventCorpseLocated {
                id,
                position: event_point3(position),
            });
            push_ref(batch, ffi::SessionEventKind::CorpseLocated, index);
        }
        Event::ZonePoints(points) => {
            let index = batch.zone_points.len();
            batch.zone_points.push(ffi::EventZonePoints {
                points: points.into_iter().map(event_zone_point).collect(),
            });
            push_ref(batch, ffi::SessionEventKind::ZonePoints, index);
        }
        Event::Combat {
            source,
            target,
            kind,
            damage,
            spell_id,
        } => {
            let index = batch.combat.len();
            batch.combat.push(ffi::EventCombat {
                source,
                target,
                kind,
                damage,
                spell_id,
            });
            push_ref(batch, ffi::SessionEventKind::Combat, index);
        }
        Event::SpawnCast {
            caster_id,
            spell_id,
            cast_time_ms,
        } => {
            let index = batch.spawn_cast.len();
            batch.spawn_cast.push(ffi::EventSpawnCast {
                caster_id,
                spell_id,
                cast_time_ms,
            });
            push_ref(batch, ffi::SessionEventKind::SpawnCast, index);
        }
        Event::Targeted { spawn_id } => {
            let index = batch.spawn_id.len();
            batch.spawn_id.push(ffi::EventSpawnId { id: spawn_id });
            push_ref(batch, ffi::SessionEventKind::Targeted, index);
        }
        Event::Considered { spawn_id } => {
            let index = batch.spawn_id.len();
            batch.spawn_id.push(ffi::EventSpawnId { id: spawn_id });
            push_ref(batch, ffi::SessionEventKind::Considered, index);
        }
        Event::AaTable { desc_id, title_sid } => {
            let index = batch.aa_table.len();
            batch
                .aa_table
                .push(ffi::EventAaTable { desc_id, title_sid });
            push_ref(batch, ffi::SessionEventKind::AaTable, index);
        }
        Event::AlternateAbilityDefined(definition) => {
            let index = batch.alternate_ability_defined.len();
            batch
                .alternate_ability_defined
                .push(ffi::EventAlternateAbilityDefinition {
                    ability_id: definition.ability_id,
                    title_string_id: definition.title_string_id,
                });
            push_ref(batch, ffi::SessionEventKind::AlternateAbilityDefined, index);
        }
        Event::Exp { exp } => {
            let index = batch.exp.len();
            batch.exp.push(ffi::EventExp { exp });
            push_ref(batch, ffi::SessionEventKind::Exp, index);
        }
        Event::ExperienceUpdated(progress) => {
            let index = batch.experience_updated.len();
            batch.experience_updated.push(ffi::EventExperienceProgress {
                experience: progress.experience,
                has_level: progress.level.is_some(),
                level: progress.level.unwrap_or_default(),
                has_previous_level: progress.previous_level.is_some(),
                previous_level: progress.previous_level.unwrap_or_default(),
            });
            push_ref(batch, ffi::SessionEventKind::ExperienceUpdated, index);
        }
        Event::AaExp { alt_exp, aa_points } => {
            let index = batch.aa_exp.len();
            batch.aa_exp.push(ffi::EventAaExp { alt_exp, aa_points });
            push_ref(batch, ffi::SessionEventKind::AaExp, index);
        }
        Event::AlternateAdvancementSnapshot(snapshot) => {
            let index = batch.alternate_advancement_snapshot.len();
            batch
                .alternate_advancement_snapshot
                .push(ffi::EventAlternateAdvancementSnapshot {
                    purchased: snapshot
                        .purchased
                        .into_iter()
                        .map(|rank| ffi::EventAlternateAbilityRank {
                            ability_id: rank.ability_id,
                            rank: rank.rank,
                        })
                        .collect(),
                    has_spent_points: snapshot.spent_points.is_some(),
                    spent_points: snapshot.spent_points.unwrap_or_default(),
                    has_assigned_points: snapshot.assigned_points.is_some(),
                    assigned_points: snapshot.assigned_points.unwrap_or_default(),
                    unspent_points: snapshot.unspent_points,
                    experience: snapshot.experience,
                });
            push_ref(
                batch,
                ffi::SessionEventKind::AlternateAdvancementSnapshot,
                index,
            );
        }
        Event::AlternateAdvancementUpdated(progress) => {
            let index = batch.alternate_advancement_updated.len();
            batch
                .alternate_advancement_updated
                .push(ffi::EventAlternateAdvancementProgress {
                    experience: progress.experience,
                    unspent_points: progress.unspent_points,
                });
            push_ref(
                batch,
                ffi::SessionEventKind::AlternateAdvancementUpdated,
                index,
            );
        }
        Event::Stamina { food, water } => {
            let index = batch.stamina.len();
            batch.stamina.push(ffi::EventStaminaPayload { food, water });
            push_ref(batch, ffi::SessionEventKind::Stamina, index);
        }
        Event::ManaUpdate { mana } => {
            let index = batch.mana_update.len();
            batch.mana_update.push(ffi::EventManaUpdate { mana });
            push_ref(batch, ffi::SessionEventKind::ManaUpdate, index);
        }
        Event::SkillUpdate { skill_id, value } => {
            let index = batch.skill_update.len();
            batch
                .skill_update
                .push(ffi::EventSkillUpdatePayload { skill_id, value });
            push_ref(batch, ffi::SessionEventKind::SkillUpdate, index);
        }
        Event::SkillsSnapshot { skills } => {
            let index = batch.skills_snapshot.len();
            batch.skills_snapshot.push(ffi::EventSkillsSnapshot {
                skills: skills
                    .into_iter()
                    .map(|skill| ffi::EventSkillValue {
                        skill_id: skill.skill_id,
                        value: skill.value,
                    })
                    .collect(),
            });
            push_ref(batch, ffi::SessionEventKind::SkillsSnapshot, index);
        }
        Event::SkillValueUpdated(skill) => {
            let index = batch.skill_value_updated.len();
            batch.skill_value_updated.push(ffi::EventSkillValue {
                skill_id: skill.skill_id,
                value: skill.value,
            });
            push_ref(batch, ffi::SessionEventKind::SkillValueUpdated, index);
        }
        Event::LootTransaction {
            corpse_id,
            item_id,
            quantity,
            coin_copper,
            from_corpse,
        } => {
            let index = batch.loot_transaction.len();
            batch
                .loot_transaction
                .push(ffi::EventLootTransactionPayload {
                    corpse_id,
                    item_id,
                    quantity,
                    coin_copper,
                    from_corpse,
                });
            push_ref(batch, ffi::SessionEventKind::LootTransaction, index);
        }
        Event::LootDrops {
            corpse_id,
            corpse_name,
            items,
        } => {
            let index = batch.loot_drops.len();
            batch.loot_drops.push(ffi::EventLootDropsPayload {
                corpse_id,
                corpse_name,
                items: items.into_iter().map(event_loot_item).collect(),
            });
            push_ref(batch, ffi::SessionEventKind::LootDrops, index);
        }
        Event::Money {
            platinum,
            gold,
            silver,
            copper,
        } => {
            let index = batch.money.len();
            batch.money.push(ffi::EventMoney {
                platinum,
                gold,
                silver,
                copper,
            });
            push_ref(batch, ffi::SessionEventKind::Money, index);
        }
        Event::MoneyBalanceUpdated(balance) => {
            let index = batch.money_balance_updated.len();
            batch.money_balance_updated.push(ffi::EventMoneyBalance {
                platinum: balance.platinum,
                gold: balance.gold,
                silver: balance.silver,
                copper: balance.copper,
            });
            push_ref(batch, ffi::SessionEventKind::MoneyBalanceUpdated, index);
        }
        Event::SimpleMessage { format_id, color } => {
            let index = batch.simple_message.len();
            batch
                .simple_message
                .push(ffi::EventSimpleMessagePayload { format_id, color });
            push_ref(batch, ffi::SessionEventKind::SimpleMessage, index);
        }
        Event::FormattedMessage {
            format_id,
            color,
            args,
        } => {
            let index = batch.formatted_message.len();
            batch
                .formatted_message
                .push(ffi::EventFormattedMessagePayload {
                    format_id,
                    color,
                    args,
                });
            push_ref(batch, ffi::SessionEventKind::FormattedMessage, index);
        }
        Event::SpecialMessage {
            color,
            target,
            source,
            message,
        } => {
            let index = batch.special_message.len();
            batch.special_message.push(ffi::EventSpecialMessagePayload {
                color,
                target,
                source,
                message,
            });
            push_ref(batch, ffi::SessionEventKind::SpecialMessage, index);
        }
        Event::LootMessage {
            color,
            text,
            item_id,
            item_name,
        } => {
            let index = batch.loot_message.len();
            batch.loot_message.push(ffi::EventLootMessagePayload {
                color,
                text,
                item_id,
                item_name,
            });
            push_ref(batch, ffi::SessionEventKind::LootMessage, index);
        }
        Event::Chat {
            channel,
            from,
            target,
            text,
            chat_color,
            channel_name,
        } => {
            let index = batch.chat.len();
            batch.chat.push(ffi::EventChat {
                channel,
                from,
                target,
                text,
                chat_color,
                channel_name,
            });
            push_ref(batch, ffi::SessionEventKind::Chat, index);
        }
        Event::BuffList { owner, entries } => {
            let index = batch.buff_list.len();
            batch.buff_list.push(ffi::EventBuffList {
                owner,
                entries: entries.into_iter().map(event_buff).collect(),
            });
            push_ref(batch, ffi::SessionEventKind::BuffList, index);
        }
        Event::GroupFollow { name, level } => {
            let index = batch.group_follow.len();
            batch
                .group_follow
                .push(ffi::EventGroupFollowPayload { name, level });
            push_ref(batch, ffi::SessionEventKind::GroupFollow, index);
        }
        Event::GroupDisband {
            yourname,
            membername,
        } => {
            let index = batch.group_disband.len();
            batch.group_disband.push(ffi::EventGroupDisbandPayload {
                yourname,
                membername,
            });
            push_ref(batch, ffi::SessionEventKind::GroupDisband, index);
        }
        Event::LevelUpdate {
            level,
            level_old,
            exp,
        } => {
            let index = batch.level_update.len();
            batch.level_update.push(ffi::EventLevelUpdatePayload {
                level,
                level_old,
                exp,
            });
            push_ref(batch, ffi::SessionEventKind::LevelUpdate, index);
        }
        Event::EnterWorld { character_name } => {
            let index = batch.enter_world.len();
            batch
                .enter_world
                .push(ffi::EventEnterWorld { character_name });
            push_ref(batch, ffi::SessionEventKind::EnterWorld, index);
        }
    }
}

fn empty_session_batch(
    protocol_generation: u64,
    disposition: ffi::SessionDisposition,
) -> ffi::SessionDecodeBatch {
    ffi::SessionDecodeBatch {
        protocol_generation,
        disposition,
        events: Vec::new(),
        player_identity_updated: Vec::new(),
        player_moved: Vec::new(),
        player_vitals_updated: Vec::new(),
        spawn_health_updated: Vec::new(),
        player_died: Vec::new(),
        spawn_died: Vec::new(),
        spawn_identity_updated: Vec::new(),
        player_appearance_updated: Vec::new(),
        spawn_added: Vec::new(),
        spawn_moved: Vec::new(),
        spawn_removed: Vec::new(),
        spawn_renamed: Vec::new(),
        spawn_killed: Vec::new(),
        spawn_hp: Vec::new(),
        stat_sync: Vec::new(),
        self_pos: Vec::new(),
        spawn_animation: Vec::new(),
        spawn_illusion: Vec::new(),
        guilds_in_zone: Vec::new(),
        time_of_day: Vec::new(),
        zone_changed: Vec::new(),
        session_reset: Vec::new(),
        zone_transition: Vec::new(),
        zone_environment_changed: Vec::new(),
        player_profile: Vec::new(),
        named: Vec::new(),
        inspect_answer: Vec::new(),
        guild_roster: Vec::new(),
        zone_server_info: Vec::new(),
        item_set: Vec::new(),
        item_learned: Vec::new(),
        inventory_snapshot: Vec::new(),
        inventory_item_updated: Vec::new(),
        equipment_snapshot: Vec::new(),
        equipment_slot_updated: Vec::new(),
        guild_motd: Vec::new(),
        guild_rank_name: Vec::new(),
        loadout_swap: Vec::new(),
        doors: Vec::new(),
        ground_item_removed: Vec::new(),
        ground_item: Vec::new(),
        corpse_located: Vec::new(),
        zone_points: Vec::new(),
        combat: Vec::new(),
        spawn_cast: Vec::new(),
        spawn_id: Vec::new(),
        aa_table: Vec::new(),
        alternate_ability_defined: Vec::new(),
        exp: Vec::new(),
        experience_updated: Vec::new(),
        aa_exp: Vec::new(),
        alternate_advancement_snapshot: Vec::new(),
        alternate_advancement_updated: Vec::new(),
        stamina: Vec::new(),
        mana_update: Vec::new(),
        skill_update: Vec::new(),
        skills_snapshot: Vec::new(),
        skill_value_updated: Vec::new(),
        loot_transaction: Vec::new(),
        loot_drops: Vec::new(),
        money: Vec::new(),
        money_balance_updated: Vec::new(),
        simple_message: Vec::new(),
        formatted_message: Vec::new(),
        special_message: Vec::new(),
        loot_message: Vec::new(),
        chat: Vec::new(),
        buff_list: Vec::new(),
        group_follow: Vec::new(),
        group_disband: Vec::new(),
        level_update: Vec::new(),
        enter_world: Vec::new(),
        self_stats: Vec::new(),
        loot_rows: Vec::new(),
    }
}

fn struct_size_overrides() -> Vec<ffi::StructSize> {
    // The daemon's SZC_Match size table is built from Live's C++ `sizeof`
    // (s_everquest.h); these entries let the linked backend override any name
    // whose wire size diverges. live/test diverge from nothing → empty; eql
    // sources its list from the pinned seq-backend-eql struct/parser sizes.
    #[cfg(feature = "backend-eql")]
    let raw = seq_backend_eql::size_overrides();
    #[cfg(not(feature = "backend-eql"))]
    let raw: Vec<(&'static str, u32)> = Vec::new();
    raw.into_iter()
        .map(|(name, size)| ffi::StructSize {
            name: name.to_string(),
            size,
        })
        .collect()
}

fn door_stride() -> usize {
    backend::spawn_door::PAYLOAD_LEN
}

fn decode_mob_update(bytes: &[u8]) -> ffi::MobUpdate {
    // eql's OP_MobUpdate is byte-identical to Live's spawnPositionUpdate
    // (14B, packed y:19/z:19/u3:7/x:19/heading:12 fixed-point ×8; verified
    // 2026-07-08 over 1665 packets — 19-bit sign-fill consistent on every
    // axis), so every backend shares the Live parser here.
    match backend::parse_mob_update(bytes) {
        Ok(m) => ffi::MobUpdate {
            spawn_id: m.spawn_id,
            x: m.x,
            y: m.y,
            z: m.z,
            heading: m.heading,
            ok: true,
        },
        Err(_) => ffi::MobUpdate {
            spawn_id: 0,
            x: 0,
            y: 0,
            z: 0,
            heading: 0,
            ok: false,
        },
    }
}

fn decode_delete_spawn(bytes: &[u8]) -> ffi::DeleteSpawn {
    match backend::parse_delete_spawn(bytes) {
        Ok(d) => ffi::DeleteSpawn {
            spawn_id: d.spawn_id,
            ok: true,
        },
        Err(_) => ffi::DeleteSpawn {
            spawn_id: 0,
            ok: false,
        },
    }
}

fn decode_remove_spawn(bytes: &[u8]) -> ffi::RemoveSpawn {
    match backend::parse_remove_spawn(bytes) {
        Ok(r) => ffi::RemoveSpawn {
            spawn_id: r.spawn_id,
            remove_spawn: r.remove_spawn,
            ok: true,
        },
        Err(_) => ffi::RemoveSpawn {
            spawn_id: 0,
            remove_spawn: 0,
            ok: false,
        },
    }
}

#[cfg(not(feature = "backend-eql"))]
fn decode_hp_update(bytes: &[u8]) -> ffi::HpUpdate {
    match backend::parse_hp_update(bytes) {
        Ok(h) => ffi::HpUpdate {
            spawn_id: h.spawn_id,
            cur_hp: h.cur_hp,
            max_hp: h.max_hp,
            ok: true,
        },
        Err(_) => ffi::HpUpdate {
            spawn_id: 0,
            cur_hp: 0,
            max_hp: 0,
            ok: false,
        },
    }
}

// eql: OP_HPUpdate is the multiplexed stat-sync channel, decoded via
// decode_stat_sync — Live's fixed HP struct never appears, so this shared FFI
// is inert.
#[cfg(feature = "backend-eql")]
fn decode_hp_update(_bytes: &[u8]) -> ffi::HpUpdate {
    ffi::HpUpdate {
        spawn_id: 0,
        cur_hp: 0,
        max_hp: 0,
        ok: false,
    }
}

fn stat_sync_err() -> ffi::StatSync {
    ffi::StatSync {
        spawn_id: 0,
        wide: false,
        has_hp: false,
        hp_cur: 0,
        hp_max: 0,
        has_mana: false,
        mana_cur: 0,
        mana_max: 0,
        has_end: false,
        end_cur: 0,
        end_max: 0,
        ok: false,
    }
}

// eql-only: the multiplexed stat-sync channel (real HP cur/max + player mana).
// live/test have no such channel, so their build gets an inert stub.
#[cfg(feature = "backend-eql")]
fn decode_stat_sync(bytes: &[u8]) -> ffi::StatSync {
    match seq_backend_eql::parse_stat_sync(bytes) {
        Ok(s) => ffi::StatSync {
            spawn_id: s.spawn_id,
            wide: s.wide,
            has_hp: s.has_hp,
            hp_cur: s.hp_cur,
            hp_max: s.hp_max,
            has_mana: s.has_mana,
            mana_cur: s.mana_cur,
            mana_max: s.mana_max,
            has_end: s.has_end,
            end_cur: s.end_cur,
            end_max: s.end_max,
            ok: true,
        },
        Err(_) => stat_sync_err(),
    }
}

// eql session identity — the one stateful thing on this bridge. See
// seq_backend_eql::self_track for why it can't be a pure per-packet function.
// The C++/Elixir side owns one per session; all logic stays in the backend so
// the host only forwards packets and applies the verdict.
#[cfg(feature = "backend-eql")]
pub struct EqlSelfTracker(seq_backend_eql::SelfTracker);
#[cfg(not(feature = "backend-eql"))]
pub struct EqlSelfTracker;

#[cfg(not(feature = "backend-eql"))]
fn self_stat_none() -> ffi::SelfStat {
    ffi::SelfStat {
        is_self: false,
        has_hp: false,
        hp_cur: 0,
        hp_max: 0,
        has_mana: false,
        mana_cur: 0,
        mana_max: 0,
        has_end: false,
        end_cur: 0,
        end_max: 0,
    }
}

#[cfg(feature = "backend-eql")]
fn self_stat_to_ffi(v: seq_backend_eql::SelfStat) -> ffi::SelfStat {
    ffi::SelfStat {
        is_self: v.is_self,
        has_hp: v.has_hp,
        hp_cur: v.hp_cur,
        hp_max: v.hp_max,
        has_mana: v.has_mana,
        mana_cur: v.mana_cur,
        mana_max: v.mana_max,
        has_end: v.has_end,
        end_cur: v.end_cur,
        end_max: v.end_max,
    }
}

#[cfg(feature = "backend-eql")]
fn eql_self_tracker_new() -> Box<EqlSelfTracker> {
    Box::new(EqlSelfTracker(seq_backend_eql::SelfTracker::new()))
}

#[cfg(feature = "backend-eql")]
impl EqlSelfTracker {
    fn reset(&mut self) {
        self.0.reset();
    }

    fn self_id(&self) -> u32 {
        self.0.self_id()
    }

    fn observe_spawn(&mut self, player_name: &str, spawn_name: &str, spawn_id: u32) -> u8 {
        self.0.observe_spawn(player_name, spawn_name, spawn_id) as u8
    }

    fn observe_stat_sync(&mut self, stat: &ffi::StatSync) -> ffi::SelfStat {
        let s = seq_backend_eql::StatSync {
            spawn_id: stat.spawn_id,
            wide: stat.wide,
            has_hp: stat.has_hp,
            hp_cur: stat.hp_cur,
            hp_max: stat.hp_max,
            has_mana: stat.has_mana,
            mana_cur: stat.mana_cur,
            mana_max: stat.mana_max,
            has_end: stat.has_end,
            end_cur: stat.end_cur,
            end_max: stat.end_max,
        };
        self_stat_to_ffi(self.0.observe_stat_sync(&s))
    }

    fn take_pending_vitals(&mut self) -> ffi::SelfStat {
        self_stat_to_ffi(self.0.take_pending_vitals())
    }

    fn observe_self_pos(&mut self, spawn_id: u32) -> u8 {
        self.0.observe_self_pos(spawn_id) as u8
    }

    fn provisional_id(&self) -> u32 {
        self.0.provisional_id()
    }

    fn take_retired_provisional(&mut self) -> u32 {
        self.0.take_retired_provisional()
    }
}

// live/test never see the eql self-record pair, so the tracker is inert there.
#[cfg(not(feature = "backend-eql"))]
fn eql_self_tracker_new() -> Box<EqlSelfTracker> {
    Box::new(EqlSelfTracker)
}

#[cfg(not(feature = "backend-eql"))]
impl EqlSelfTracker {
    fn reset(&mut self) {}
    fn self_id(&self) -> u32 {
        0
    }
    fn observe_spawn(&mut self, _player_name: &str, _spawn_name: &str, _spawn_id: u32) -> u8 {
        0
    }
    fn observe_stat_sync(&mut self, _stat: &ffi::StatSync) -> ffi::SelfStat {
        self_stat_none()
    }
    fn take_pending_vitals(&mut self) -> ffi::SelfStat {
        self_stat_none()
    }
    fn observe_self_pos(&mut self, _spawn_id: u32) -> u8 {
        0
    }
    fn provisional_id(&self) -> u32 {
        0
    }
    fn take_retired_provisional(&mut self) -> u32 {
        0
    }
}

// Loot recording state. Same shape as EqlSelfTracker above: the host owns one
// per session and only forwards packets; all the pairing lives in the backend
// so every host inherits identical behaviour.
#[cfg(feature = "backend-eql")]
pub struct EqlLootTracker(seq_backend_eql::LootTracker);
#[cfg(not(feature = "backend-eql"))]
pub struct EqlLootTracker;

#[cfg(feature = "backend-eql")]
fn eql_loot_tracker_new() -> Box<EqlLootTracker> {
    Box::new(EqlLootTracker(seq_backend_eql::LootTracker::new()))
}

#[cfg(feature = "backend-eql")]
fn loot_rows_to_ffi(rows: Vec<seq_backend_eql::LootRow>) -> Vec<ffi::LootRow> {
    rows.into_iter()
        .map(|r| ffi::LootRow {
            ts: r.ts,
            source: r.source.as_str().to_string(),
            item_name: r.item_name,
            item_id: r.item_id,
            icon: r.icon,
            qty: r.qty,
            mob_name: r.mob_name,
            mob_norm: r.mob_norm,
            corpse_id: r.corpse_id,
            zone_short: r.zone_short,
            zone_base: r.zone_base,
            instance: r.instance,
            sold: r.sold,
            money_copper: r.money_copper,
            disposition: r.disposition,
            looter: r.looter,
            sequence: r.sequence,
        })
        .collect()
}

#[cfg(feature = "backend-eql")]
impl EqlLootTracker {
    fn reset(&mut self) {
        self.0.reset();
    }
    fn set_looter(&mut self, looter: &str) {
        self.0.set_looter(looter);
    }
    fn set_zone(&mut self, zone_short: &str) -> Vec<ffi::LootRow> {
        loot_rows_to_ffi(self.0.set_zone(zone_short))
    }
    fn on_loot_message(
        &mut self,
        color: u32,
        text: &str,
        item_id: u32,
        item_name: &str,
        ts: i64,
    ) -> Vec<ffi::LootRow> {
        loot_rows_to_ffi(self.0.on_loot_message(color, text, item_id, item_name, ts))
    }
    fn on_loot_transaction(&mut self, t: &ffi::LootTransaction, ts: i64) -> Vec<ffi::LootRow> {
        loot_rows_to_ffi(self.0.on_loot_transaction(
            t.corpse_id,
            t.item_id,
            t.quantity,
            t.coin_copper,
            t.from_corpse,
            t.sequence,
            ts,
        ))
    }
    fn on_loot_drop_item(
        &mut self,
        corpse_id: u32,
        corpse_name: &str,
        item_name: &str,
        icon: u32,
        item_id: u32,
        ts: i64,
    ) -> Vec<ffi::LootRow> {
        loot_rows_to_ffi(self.0.on_loot_drop_item(
            corpse_id,
            corpse_name,
            item_name,
            icon,
            item_id,
            ts,
        ))
    }
    fn flush(&mut self) -> Vec<ffi::LootRow> {
        loot_rows_to_ffi(self.0.flush())
    }
}

#[cfg(not(feature = "backend-eql"))]
fn eql_loot_tracker_new() -> Box<EqlLootTracker> {
    Box::new(EqlLootTracker)
}

#[cfg(not(feature = "backend-eql"))]
impl EqlLootTracker {
    fn reset(&mut self) {}
    fn set_looter(&mut self, _looter: &str) {}
    fn set_zone(&mut self, _zone_short: &str) -> Vec<ffi::LootRow> {
        Vec::new()
    }
    fn on_loot_message(
        &mut self,
        _color: u32,
        _text: &str,
        _item_id: u32,
        _item_name: &str,
        _ts: i64,
    ) -> Vec<ffi::LootRow> {
        Vec::new()
    }
    fn on_loot_transaction(&mut self, _t: &ffi::LootTransaction, _ts: i64) -> Vec<ffi::LootRow> {
        Vec::new()
    }
    fn on_loot_drop_item(
        &mut self,
        _corpse_id: u32,
        _corpse_name: &str,
        _item_name: &str,
        _icon: u32,
        _item_id: u32,
        _ts: i64,
    ) -> Vec<ffi::LootRow> {
        Vec::new()
    }
    fn flush(&mut self) -> Vec<ffi::LootRow> {
        Vec::new()
    }
}

// eql-only: OP_BeginCast — a spawn started casting a spell. live/test
// have no such opcode wired, so their build gets an inert stub.
#[cfg(feature = "backend-eql")]
fn decode_begin_cast(bytes: &[u8]) -> ffi::BeginCast {
    match seq_backend_eql::parse_begin_cast(bytes) {
        Ok(c) => ffi::BeginCast {
            caster_id: c.caster_id,
            spell_id: c.spell_id,
            cast_time_ms: c.cast_time_ms,
            ok: true,
        },
        Err(_) => ffi::BeginCast {
            caster_id: 0,
            spell_id: 0,
            cast_time_ms: 0,
            ok: false,
        },
    }
}
#[cfg(not(feature = "backend-eql"))]
fn decode_begin_cast(_bytes: &[u8]) -> ffi::BeginCast {
    ffi::BeginCast {
        caster_id: 0,
        spell_id: 0,
        cast_time_ms: 0,
        ok: false,
    }
}

// eql-only: OP_Stance / OP_Invocation — 4B {u32 abilityId}.
// live/test have no such opcode wired, so their build gets an inert stub.
#[cfg(feature = "backend-eql")]
fn decode_activate_ability(bytes: &[u8]) -> ffi::ActivateAbility {
    match seq_backend_eql::parse_activate_ability(bytes) {
        Ok(id) => ffi::ActivateAbility {
            ability_id: id,
            ok: true,
        },
        Err(_) => ffi::ActivateAbility {
            ability_id: 0,
            ok: false,
        },
    }
}
#[cfg(not(feature = "backend-eql"))]
fn decode_activate_ability(_bytes: &[u8]) -> ffi::ActivateAbility {
    ffi::ActivateAbility {
        ability_id: 0,
        ok: false,
    }
}

// eql-only: OP_SendAATable — one AA ability-rank definition. live/test
// have no such opcode wired, so their build gets an inert stub.
#[cfg(feature = "backend-eql")]
fn decode_aa_table_entry(bytes: &[u8]) -> ffi::AaTableEntry {
    match seq_backend_eql::parse_aa_table_entry(bytes) {
        Ok(e) => ffi::AaTableEntry {
            desc_id: e.desc_id,
            title_sid: e.title_sid,
            ok: true,
        },
        Err(_) => ffi::AaTableEntry {
            desc_id: 0,
            title_sid: 0,
            ok: false,
        },
    }
}
#[cfg(not(feature = "backend-eql"))]
fn decode_aa_table_entry(_bytes: &[u8]) -> ffi::AaTableEntry {
    ffi::AaTableEntry {
        desc_id: 0,
        title_sid: 0,
        ok: false,
    }
}

fn loadout_swap_err() -> ffi::LoadoutSwap {
    ffi::LoadoutSwap {
        spawn_id: 0,
        level: 0,
        class_: 0,
        race: 0,
        ok: false,
    }
}

// eql-only: OP_LoadoutSwap. live/test have no such opcode, so their
// build gets an inert stub.
#[cfg(feature = "backend-eql")]
fn decode_loadout_swap(bytes: &[u8]) -> ffi::LoadoutSwap {
    match seq_backend_eql::parse_loadout_swap(bytes) {
        Ok(s) => ffi::LoadoutSwap {
            spawn_id: s.spawn_id,
            level: s.level,
            class_: s.class_,
            race: s.race,
            ok: true,
        },
        Err(_) => loadout_swap_err(),
    }
}

#[cfg(not(feature = "backend-eql"))]
fn decode_loadout_swap(_bytes: &[u8]) -> ffi::LoadoutSwap {
    loadout_swap_err()
}

const LOOT_TXN_NONE: ffi::LootTransaction = ffi::LootTransaction {
    item_id: 0,
    slot: 0,
    corpse_id: 0,
    quantity: 0,
    sequence: 0,
    coin_copper: 0,
    from_corpse: false,
    ok: false,
};

// eql-only: OP_LootTransaction subcode-7 item confirmation or
// subcode-5 corpse coin pile.
#[cfg(feature = "backend-eql")]
fn decode_loot_transaction(bytes: &[u8]) -> ffi::LootTransaction {
    match seq_backend_eql::parse_loot_transaction(bytes) {
        Ok(t) => ffi::LootTransaction {
            item_id: t.item_id,
            slot: t.slot,
            corpse_id: t.corpse_id,
            quantity: t.quantity,
            sequence: t.sequence,
            coin_copper: t.coin_copper,
            from_corpse: t.from_corpse,
            ok: true,
        },
        Err(_) => LOOT_TXN_NONE,
    }
}

#[cfg(not(feature = "backend-eql"))]
fn decode_loot_transaction(_bytes: &[u8]) -> ffi::LootTransaction {
    LOOT_TXN_NONE
}

const MONEY_NONE: ffi::MoneyUpdate = ffi::MoneyUpdate {
    platinum: 0,
    gold: 0,
    silver: 0,
    copper: 0,
    ok: false,
};

// eql-only: OP_MoneyUpdate carried purse, four denominations.
#[cfg(feature = "backend-eql")]
fn decode_money_update(bytes: &[u8]) -> ffi::MoneyUpdate {
    match seq_backend_eql::parse_money_update(bytes) {
        Ok(m) => ffi::MoneyUpdate {
            platinum: m.platinum,
            gold: m.gold,
            silver: m.silver,
            copper: m.copper,
            ok: true,
        },
        Err(_) => MONEY_NONE,
    }
}

#[cfg(not(feature = "backend-eql"))]
fn decode_money_update(_bytes: &[u8]) -> ffi::MoneyUpdate {
    MONEY_NONE
}

// eql-only: OP_LootDrops corpse loot window.
#[cfg(feature = "backend-eql")]
fn decode_loot_drops(bytes: &[u8]) -> ffi::LootDrops {
    match seq_backend_eql::parse_loot_drops(bytes) {
        Ok(l) => ffi::LootDrops {
            corpse_id: l.corpse_id,
            corpse_name: l.corpse_name,
            items: l
                .items
                .into_iter()
                .map(|it| ffi::LootItem {
                    name: it.name,
                    icon: it.icon,
                    item_id: it.item_id,
                })
                .collect(),
            ok: true,
        },
        Err(_) => ffi::LootDrops {
            corpse_id: 0,
            corpse_name: String::new(),
            items: Vec::new(),
            ok: false,
        },
    }
}

#[cfg(not(feature = "backend-eql"))]
fn decode_loot_drops(_bytes: &[u8]) -> ffi::LootDrops {
    ffi::LootDrops {
        corpse_id: 0,
        corpse_name: String::new(),
        items: Vec::new(),
        ok: false,
    }
}

// eql-only: OP_LootMessage personal loot text, reusing SpecialMessage.
#[cfg(feature = "backend-eql")]
fn decode_loot_message(bytes: &[u8]) -> ffi::LootMessage {
    match seq_backend_eql::parse_loot_message(bytes) {
        Ok(m) => ffi::LootMessage {
            color: m.color,
            text: m.text,
            item_id: m.item_id,
            item_name: m.item_name,
            ok: true,
        },
        Err(_) => loot_message_none(),
    }
}

fn loot_message_none() -> ffi::LootMessage {
    ffi::LootMessage {
        color: 0,
        text: String::new(),
        item_id: 0,
        item_name: String::new(),
        ok: false,
    }
}

#[cfg(not(feature = "backend-eql"))]
fn decode_loot_message(_bytes: &[u8]) -> ffi::LootMessage {
    loot_message_none()
}

#[cfg(not(feature = "backend-eql"))]
fn decode_stat_sync(_bytes: &[u8]) -> ffi::StatSync {
    stat_sync_err()
}

// eql-only: OP_GuildsInZoneList / OP_NewGuildInZone — the guilds present in the
// zone, the only source of guild NAMES. eql owns this parser like every other;
// the daemon and scry both consume it rather than each re-decoding the wire.
// live/test stub empty.
#[cfg(feature = "backend-eql")]
fn decode_guilds_in_zone_list(bytes: &[u8]) -> Vec<ffi::GuildInZoneRow> {
    match seq_backend_eql::guild_in_zone::parse_guilds_in_zone_list(bytes) {
        Ok(list) => list
            .into_iter()
            .map(|g| ffi::GuildInZoneRow {
                guild_id: g.guild_id,
                server_id: g.server_id,
                name: g.name,
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

#[cfg(not(feature = "backend-eql"))]
fn decode_guilds_in_zone_list(bytes: &[u8]) -> Vec<ffi::GuildInZoneRow> {
    match seq_decode::guild_in_zone::parse_guilds_in_zone_list(bytes) {
        Ok(list) => list
            .into_iter()
            .map(|g| ffi::GuildInZoneRow {
                guild_id: g.guild_id,
                server_id: g.server_id,
                name: g.name,
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

#[cfg(feature = "backend-eql")]
fn decode_new_guild_in_zone(bytes: &[u8]) -> Vec<ffi::GuildInZoneRow> {
    match seq_backend_eql::guild_in_zone::parse_new_guild_in_zone(bytes) {
        Ok(g) => vec![ffi::GuildInZoneRow {
            guild_id: g.guild_id,
            server_id: g.server_id,
            name: g.name,
        }],
        Err(_) => Vec::new(),
    }
}

#[cfg(not(feature = "backend-eql"))]
fn decode_new_guild_in_zone(bytes: &[u8]) -> Vec<ffi::GuildInZoneRow> {
    match seq_decode::guild_in_zone::parse_new_guild_in_zone(bytes) {
        Ok(g) => vec![ffi::GuildInZoneRow {
            guild_id: g.guild_id,
            server_id: g.server_id,
            name: g.name,
        }],
        Err(_) => Vec::new(),
    }
}

// OP_GuildMOTD — the guild message of the day. Each backend owns its parser
// (`backend` = seq_decode for live/test, seq_backend_eql for eql); the wire is
// the stock struct on both today.
fn decode_guild_motd(bytes: &[u8]) -> ffi::GuildMotd {
    match backend::guild_motd::parse_guild_motd(bytes) {
        Ok(m) => ffi::GuildMotd {
            message: m.message,
            sender: m.sender,
            ok: true,
        },
        Err(_) => ffi::GuildMotd {
            message: String::new(),
            sender: String::new(),
            ok: false,
        },
    }
}

// OP_ExpandedGuildInfo (Live/Test) — one rank-name-table entry. eql's guild
// wire diverges and it has no parser for this yet, so it stubs action 0.
#[cfg(not(feature = "backend-eql"))]
fn decode_guild_expanded_info(bytes: &[u8]) -> ffi::GuildExpandedInfo {
    let i = seq_decode::guild_expanded_info::parse_expanded_guild_info(bytes);
    ffi::GuildExpandedInfo {
        action: i.action,
        guild_id: i.guild_id,
        rank_index: i.rank_index,
        rank_name: i.rank_name,
    }
}

#[cfg(feature = "backend-eql")]
fn decode_guild_expanded_info(_bytes: &[u8]) -> ffi::GuildExpandedInfo {
    ffi::GuildExpandedInfo {
        action: 0,
        guild_id: 0,
        rank_index: 0,
        rank_name: String::new(),
    }
}

// OP_GuildMemberUpdate (Live/Test) — one member's zone/last-on. eql's variant
// diverges and isn't wired there, so it stubs `ok:false`.
#[cfg(not(feature = "backend-eql"))]
fn decode_guild_member_update(bytes: &[u8]) -> ffi::GuildMemberUpdateInfo {
    match seq_decode::guild_member_update::parse_guild_member_update(bytes) {
        Ok(u) => ffi::GuildMemberUpdateInfo {
            name: u.name,
            zone_id: u.zone_id,
            zone_instance: u.zone_instance,
            last_on: u.last_on,
            ok: true,
        },
        Err(_) => ffi::GuildMemberUpdateInfo {
            name: String::new(),
            zone_id: 0,
            zone_instance: 0,
            last_on: 0,
            ok: false,
        },
    }
}

#[cfg(feature = "backend-eql")]
fn decode_guild_member_update(_bytes: &[u8]) -> ffi::GuildMemberUpdateInfo {
    ffi::GuildMemberUpdateInfo {
        name: String::new(),
        zone_id: 0,
        zone_instance: 0,
        last_on: 0,
        ok: false,
    }
}

// eql-only: OP_GuildMemberList — the full guild roster. Flattened to a Vec
// (empty = decode failed / empty guild). live/test stub empty: the eql wire
// diverges from the stock struct, so there is nothing shared to fall back to.
#[cfg(feature = "backend-eql")]
fn decode_guild_roster(bytes: &[u8]) -> Vec<ffi::GuildRosterRow> {
    match seq_backend_eql::guild_roster::parse_guild_member_list(bytes) {
        Ok(r) => r
            .members
            .into_iter()
            .map(|m| ffi::GuildRosterRow {
                guild_id: r.guild_id,
                primary_class: seq_backend_eql::guild_roster::primary_class(m.class_mask),
                name: m.name,
                level: m.level,
                class_mask: m.class_mask,
                rank: m.rank,
                last_on: m.last_on,
                // The wire packs both flags into one field: 0 none, 1 banker,
                // 2 alt, 3 alt banker.
                banker: (m.banker_flag % 2) as u8,
                alt: (m.banker_flag > 1) as u8,
                full_member: m.full_member,
                public_note: m.public_note,
                zone_id: m.zone_id,
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

// Live/Test: the stock roster. Single class (no multiclass mask -> class_mask 0)
// and no per-member zone (Live doesn't carry it -> zone_id 0). Its own parser
// in seq-decode, kept separate from eql's because the wire genuinely differs.
#[cfg(not(feature = "backend-eql"))]
fn decode_guild_roster(bytes: &[u8]) -> Vec<ffi::GuildRosterRow> {
    match seq_decode::guild_roster::parse_guild_member_list(bytes) {
        Ok(r) => r
            .members
            .into_iter()
            .map(|m| ffi::GuildRosterRow {
                guild_id: r.guild_id,
                primary_class: m.primary_class as u8,
                name: m.name,
                level: m.level,
                class_mask: 0,
                rank: m.rank,
                last_on: m.last_on,
                banker: m.banker as u8,
                alt: m.alt as u8,
                full_member: m.full_member as u8,
                public_note: m.public_note,
                zone_id: 0,
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

// eql-only: OP_BuffList — the authoritative per-spawn active-buff list.
// Flattened to a Vec (empty = decode failed / not present). live/test stub empty.
#[cfg(feature = "backend-eql")]
fn decode_buff_list(bytes: &[u8]) -> Vec<ffi::BuffListEntry> {
    match seq_backend_eql::parse_buff_list(bytes) {
        Ok(list) => list
            .entries
            .into_iter()
            .map(|e| ffi::BuffListEntry {
                spawn_id: list.spawn_id,
                spell_id: e.spell_id,
                remaining_ticks: e.remaining_ticks,
                slot: e.slot,
                caster: e.caster,
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

#[cfg(not(feature = "backend-eql"))]
fn decode_buff_list(_bytes: &[u8]) -> Vec<ffi::BuffListEntry> {
    Vec::new()
}

// eql-only: OP_SelfPosEQL — the local player's position-history
// breadcrumb, flattened to ordered points (empty = decode failed / not present).
// live/test stub empty.
#[cfg(feature = "backend-eql")]
fn decode_self_pos_breadcrumb(bytes: &[u8]) -> Vec<ffi::SelfPosPoint> {
    seq_backend_eql::parse_self_pos_breadcrumb(bytes)
        .points
        .into_iter()
        .map(|p| ffi::SelfPosPoint {
            x: p.x,
            y: p.y,
            z: p.z,
            ts: p.ts,
        })
        .collect()
}

#[cfg(not(feature = "backend-eql"))]
fn decode_self_pos_breadcrumb(_bytes: &[u8]) -> Vec<ffi::SelfPosPoint> {
    Vec::new()
}

// EQL UCS cross-zone chat. Flattened to a Vec (empty = no chat / not present).
// live/test stub empty.
#[cfg(feature = "backend-eql")]
fn decode_ucs_chat(bytes: &[u8]) -> Vec<ffi::UcsChatRecord> {
    seq_backend_eql::parse_ucs_chat(bytes)
        .into_iter()
        .map(|r| ffi::UcsChatRecord {
            channel_first: r.channel_first,
            channel_rest: r.channel_rest,
            channel_run: r.channel_run,
            sender: r.sender,
            message: r.message,
            spam: r.spam,
        })
        .collect()
}

#[cfg(not(feature = "backend-eql"))]
fn decode_ucs_chat(_bytes: &[u8]) -> Vec<ffi::UcsChatRecord> {
    Vec::new()
}

// EQL UCS channel-name learning (/list rosters + join notices). live/test stub.
#[cfg(feature = "backend-eql")]
fn decode_ucs_channels(bytes: &[u8]) -> Vec<String> {
    seq_backend_eql::parse_ucs_channels(bytes)
}

#[cfg(not(feature = "backend-eql"))]
fn decode_ucs_channels(_bytes: &[u8]) -> Vec<String> {
    Vec::new()
}

fn decode_mob_health(bytes: &[u8]) -> ffi::MobHealth {
    match backend::parse_mob_health(bytes) {
        Ok(m) => ffi::MobHealth {
            spawn_id: m.spawn_id,
            hp_percent: m.hp_percent,
            ok: true,
        },
        Err(_) => ffi::MobHealth {
            spawn_id: 0,
            hp_percent: 0,
            ok: false,
        },
    }
}

fn decode_spawn_appearance(bytes: &[u8]) -> ffi::SpawnAppearance {
    match backend::parse_spawn_appearance(bytes) {
        Ok(a) => ffi::SpawnAppearance {
            spawn_id: a.spawn_id,
            kind: a.kind,
            // Live's current wire carries no value field; eql's still does.
            #[cfg(feature = "backend-eql")]
            parameter: a.parameter,
            #[cfg(not(feature = "backend-eql"))]
            parameter: 0,
            ok: true,
        },
        Err(_) => ffi::SpawnAppearance {
            spawn_id: 0,
            kind: 0,
            parameter: 0,
            ok: false,
        },
    }
}

fn decode_exp_update(bytes: &[u8]) -> ffi::ExpUpdate {
    match backend::parse_exp_update(bytes) {
        Ok(e) => ffi::ExpUpdate {
            exp: e.exp,
            unknown0: e.unknown0,
            kind: e.kind,
            unknown1: e.unknown1,
            ok: true,
        },
        Err(_) => ffi::ExpUpdate {
            exp: 0,
            unknown0: 0,
            kind: 0,
            unknown1: 0,
            ok: false,
        },
    }
}

fn decode_level_update(bytes: &[u8]) -> ffi::LevelUpdate {
    match backend::parse_level_update(bytes) {
        Ok(l) => ffi::LevelUpdate {
            level: l.level,
            level_old: l.level_old,
            exp: l.exp,
            unknown0: l.unknown0,
            ok: true,
        },
        Err(_) => ffi::LevelUpdate {
            level: 0,
            level_old: 0,
            exp: 0,
            unknown0: 0,
            ok: false,
        },
    }
}

fn decode_skill_update(bytes: &[u8]) -> ffi::SkillUpdate {
    match backend::parse_skill_update(bytes) {
        Ok(s) => ffi::SkillUpdate {
            skill_id: s.skill_id,
            value: s.value,
            ok: true,
        },
        Err(_) => ffi::SkillUpdate {
            skill_id: 0,
            value: 0,
            ok: false,
        },
    }
}

fn decode_mana_change(bytes: &[u8]) -> ffi::ManaChange {
    match backend::parse_mana_change(bytes) {
        Ok(m) => ffi::ManaChange {
            new_mana: m.new_mana,
            max_mana: m.max_mana,
            spell_id: m.spell_id,
            ok: true,
        },
        Err(_) => ffi::ManaChange {
            new_mana: 0,
            max_mana: 0,
            spell_id: 0,
            ok: false,
        },
    }
}

fn decode_stamina(bytes: &[u8]) -> ffi::Stamina {
    match backend::parse_stamina(bytes) {
        Ok(s) => ffi::Stamina {
            food: s.food,
            water: s.water,
            ok: true,
        },
        Err(_) => ffi::Stamina {
            food: 0,
            water: 0,
            ok: false,
        },
    }
}

fn decode_end_update(bytes: &[u8]) -> ffi::EndUpdate {
    match backend::parse_end_update(bytes) {
        Ok(e) => ffi::EndUpdate {
            spawn_id: e.spawn_id,
            cur: e.cur,
            max: e.max,
            ok: true,
        },
        Err(_) => ffi::EndUpdate {
            spawn_id: 0,
            cur: 0,
            max: 0,
            ok: false,
        },
    }
}

fn decode_consider(bytes: &[u8]) -> ffi::Consider {
    let parsed = backend::parse_consider(bytes);
    match parsed {
        Ok(c) => ffi::Consider {
            player_id: c.player_id,
            target_id: c.target_id,
            faction: c.faction,
            level: c.level,
            ok: true,
        },
        Err(_) => ffi::Consider {
            player_id: 0,
            target_id: 0,
            faction: 0,
            level: 0,
            ok: false,
        },
    }
}

fn decode_spawn_rename(bytes: &[u8]) -> ffi::SpawnRename {
    match backend::parse_spawn_rename(bytes) {
        Ok(r) => ffi::SpawnRename {
            old_name: r.old_name,
            old_name_again: r.old_name_again,
            new_name: r.new_name,
            ok: true,
        },
        Err(_) => ffi::SpawnRename {
            old_name: String::new(),
            old_name_again: String::new(),
            new_name: String::new(),
            ok: false,
        },
    }
}

fn decode_client_target(bytes: &[u8]) -> ffi::ClientTarget {
    match backend::parse_client_target(bytes) {
        Ok(t) => ffi::ClientTarget {
            new_target: t.new_target,
            ok: true,
        },
        Err(_) => ffi::ClientTarget {
            new_target: 0,
            ok: false,
        },
    }
}

fn decode_death(bytes: &[u8]) -> ffi::Death {
    match backend::parse_death(bytes) {
        Ok(d) => ffi::Death {
            spawn_id: d.spawn_id,
            killer_id: d.killer_id,
            corpse_id: d.corpse_id,
            kind: d.kind,
            spell_id: d.spell_id,
            zone_id: d.zone_id,
            zone_instance: d.zone_instance,
            damage: d.damage,
            ok: true,
        },
        Err(_) => ffi::Death {
            spawn_id: 0,
            killer_id: 0,
            corpse_id: 0,
            kind: 0,
            spell_id: 0,
            zone_id: 0,
            zone_instance: 0,
            damage: 0,
            ok: false,
        },
    }
}

fn decode_click_object(bytes: &[u8]) -> ffi::ClickObject {
    match backend::parse_click_object(bytes) {
        Ok(c) => ffi::ClickObject {
            drop_id: c.drop_id,
            spawn_id: c.spawn_id,
            ok: true,
        },
        Err(_) => ffi::ClickObject {
            drop_id: 0,
            spawn_id: 0,
            ok: false,
        },
    }
}

fn decode_illusion(bytes: &[u8]) -> ffi::Illusion {
    match backend::parse_illusion(bytes) {
        Ok(i) => ffi::Illusion {
            spawn_id: i.spawn_id,
            name: i.name,
            race: i.race,
            gender: i.gender,
            texture: i.texture,
            helm: i.helm,
            face: i.face,
            ok: true,
        },
        Err(_) => ffi::Illusion {
            spawn_id: 0,
            name: String::new(),
            race: 0,
            gender: 0,
            texture: 0,
            helm: 0,
            face: 0,
            ok: false,
        },
    }
}

fn decode_buff(bytes: &[u8]) -> ffi::Buff {
    match backend::parse_buff(bytes) {
        Ok(b) => {
            // Only the eql wire has the 24b compact form, so only its parser
            // carries change_type; Live's Buff is left untouched.
            #[cfg(feature = "backend-eql")]
            let change_type = b.change_type;
            #[cfg(not(feature = "backend-eql"))]
            let change_type = 0u32;
            ffi::Buff {
                spawn_id: b.spawn_id,
                spell_id: b.spell_id,
                form: b.form,
                slot: b.slot,
                dur_ticks: b.dur_ticks,
                change_type,
                ok: true,
            }
        }
        Err(_) => ffi::Buff {
            spawn_id: 0,
            spell_id: 0,
            form: 0,
            slot: 0,
            dur_ticks: 0,
            change_type: 0,
            ok: false,
        },
    }
}

fn decode_action2(bytes: &[u8]) -> ffi::Action2 {
    match backend::parse_action2(bytes) {
        Ok(a) => ffi::Action2 {
            target: a.target,
            source: a.source,
            damage: a.damage,
            spell: a.spell,
            kind: a.kind,
            ok: true,
        },
        Err(_) => ffi::Action2 {
            target: 0,
            source: 0,
            damage: 0,
            spell: 0,
            kind: 0,
            ok: false,
        },
    }
}

// Zeroed sentinel for a bad/absent spawn payload. Also the field base for
// eql's partial fill (its Legends spawn only decodes id/name/pos/level/hp; the
// Live-only raw equipment/position arrays stay zero).
fn spawn_err() -> ffi::Spawn {
    ffi::Spawn {
        ok: false,
        bytes_consumed: 0,
        name: String::new(),
        last_name: String::new(),
        title: String::new(),
        suffix: String::new(),
        spawn_id: 0,
        misc_data: 0,
        body_type: 0,
        race: 0,
        deity: 0,
        guild_id: 0,
        guild_server_id: 0,
        class_: 0,
        class_mask: 0,
        pet_owner_id: 0,
        equip_data: [0; 45],
        pos_data: [0; 5],
        level: 0,
        npc: 0,
        other_data: 0,
        char_properties: 0,
        cur_hp: 0,
        holding: 0,
        state: 0,
        light: 0,
        is_mercenary: 0,
        x: 0,
        y: 0,
        z: 0,
        max_hp: 0,
        heading: 0,
    }
}

#[cfg(not(feature = "backend-eql"))]
fn decode_spawn(bytes: &[u8]) -> ffi::Spawn {
    match backend::parse_spawn(bytes) {
        Ok(s) => ffi::Spawn {
            ok: true,
            bytes_consumed: s.bytes_consumed,
            name: s.name,
            last_name: s.last_name,
            title: s.title,
            suffix: s.suffix,
            spawn_id: s.spawn_id,
            misc_data: s.misc_data,
            body_type: s.body_type,
            race: s.race,
            deity: s.deity,
            guild_id: s.guild_id,
            guild_server_id: s.guild_server_id,
            class_: s.class_,
            class_mask: 0, // live isn't multiclass
            pet_owner_id: s.pet_owner_id,
            equip_data: s.equip_data,
            pos_data: s.pos_data,
            level: s.level,
            npc: s.npc,
            other_data: s.other_data,
            char_properties: s.char_properties,
            cur_hp: s.cur_hp,
            holding: s.holding,
            state: s.state,
            light: s.light,
            is_mercenary: s.is_mercenary,
            x: 0,
            y: 0,
            z: 0,
            max_hp: 0,
            heading: 0,
        },
        Err(_) => spawn_err(),
    }
}

// eql: zone-spawn decodes id/name/decoded-pos/level/hp; the rest stays zero.
// This one keeps a cfg-split (eql's ZoneSpawn is a different shape than Live's
// Spawn — decoded x/y/z vs raw pos arrays).
#[cfg(feature = "backend-eql")]
fn decode_spawn(bytes: &[u8]) -> ffi::Spawn {
    match seq_backend_eql::parse_spawn(bytes) {
        Ok(s) => ffi::Spawn {
            ok: true,
            name: s.name,
            last_name: s.last_name,
            title: s.title,
            suffix: s.suffix,
            spawn_id: u32::from(s.id),
            race: s.race,
            class_: s.class_,
            class_mask: s.class_mask,
            deity: s.deity,
            guild_id: s.guild_id,
            guild_server_id: s.guild_server_id,
            pet_owner_id: s.pet_owner_id,
            body_type: s.body_type,
            level: s.level,
            npc: s.npc,
            holding: s.holding,
            state: s.state,
            light: s.light,
            cur_hp: s.cur_hp,
            max_hp: s.max_hp,
            x: s.x,
            y: s.y,
            z: s.z,
            heading: s.heading,
            ..spawn_err()
        },
        Err(_) => spawn_err(),
    }
}

// Stage A+6 — second small-fixed POD batch.

fn decode_wear_change(bytes: &[u8]) -> ffi::WearChange {
    match backend::parse_wear_change(bytes) {
        Ok(w) => ffi::WearChange {
            spawn_id: w.spawn_id,
            subcommand: w.subcommand,
            arg1: w.arg1,
            arg2: w.arg2,
            arg3: w.arg3,
            ok: true,
        },
        Err(_) => ffi::WearChange {
            spawn_id: 0,
            subcommand: 0,
            arg1: 0,
            arg2: 0,
            arg3: 0,
            ok: false,
        },
    }
}

fn decode_zone_change(bytes: &[u8]) -> ffi::ZoneChange {
    match backend::parse_zone_change(bytes) {
        Ok(z) => ffi::ZoneChange {
            name: z.name,
            zone_id: z.zone_id,
            zone_instance: z.zone_instance,
            ok: true,
        },
        Err(_) => ffi::ZoneChange {
            name: String::new(),
            zone_id: 0,
            zone_instance: 0,
            ok: false,
        },
    }
}

fn decode_dz_info(bytes: &[u8]) -> ffi::DzInfo {
    match backend::parse_dz_info(bytes) {
        Ok(d) => ffi::DzInfo {
            new_dz: d.new_dz,
            max_players: d.max_players,
            dz_name: d.dz_name,
            name: d.name,
            ok: true,
        },
        Err(_) => ffi::DzInfo {
            new_dz: 0,
            max_players: 0,
            dz_name: String::new(),
            name: String::new(),
            ok: false,
        },
    }
}

fn decode_dz_switch_info(bytes: &[u8]) -> ffi::DzSwitch {
    match backend::parse_dz_switch_info(bytes) {
        Ok(s) => ffi::DzSwitch {
            zone_id: s.zone_id,
            instance_id: s.instance_id,
            kind: s.kind,
            x: s.x,
            y: s.y,
            z: s.z,
            ok: true,
        },
        Err(_) => ffi::DzSwitch {
            zone_id: 0,
            instance_id: 0,
            kind: 0,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            ok: false,
        },
    }
}

fn decode_start_cast(bytes: &[u8]) -> ffi::StartCast {
    match backend::parse_start_cast(bytes) {
        Ok(s) => ffi::StartCast {
            slot: s.slot,
            spell_id: s.spell_id,
            target_id: s.target_id,
            ok: true,
        },
        Err(_) => ffi::StartCast {
            slot: 0,
            spell_id: 0,
            target_id: 0,
            ok: false,
        },
    }
}

fn decode_action(bytes: &[u8]) -> ffi::Action {
    match backend::parse_action(bytes) {
        Ok(a) => ffi::Action {
            target: a.target,
            source: a.source,
            spell: a.spell,
            level: a.level,
            kind: a.kind,
            ok: true,
        },
        Err(_) => ffi::Action {
            target: 0,
            source: 0,
            spell: 0,
            level: 0,
            kind: 0,
            ok: false,
        },
    }
}

fn decode_action_alt(bytes: &[u8]) -> ffi::Action {
    match backend::parse_action_alt(bytes) {
        Ok(a) => ffi::Action {
            target: a.target,
            source: a.source,
            spell: a.spell,
            level: a.level,
            kind: a.kind,
            ok: true,
        },
        Err(_) => ffi::Action {
            target: 0,
            source: 0,
            spell: 0,
            level: 0,
            kind: 0,
            ok: false,
        },
    }
}

fn decode_group_disband(bytes: &[u8]) -> ffi::GroupDisband {
    match backend::parse_group_disband(bytes) {
        Ok(g) => ffi::GroupDisband {
            yourname: g.yourname,
            membername: g.membername,
            ok: true,
        },
        Err(_) => ffi::GroupDisband {
            yourname: String::new(),
            membername: String::new(),
            ok: false,
        },
    }
}

fn decode_group_follow(bytes: &[u8]) -> ffi::GroupFollow {
    match backend::parse_group_follow(bytes) {
        Ok(g) => ffi::GroupFollow {
            name: g.name,
            ok: true,
        },
        Err(_) => ffi::GroupFollow {
            name: String::new(),
            ok: false,
        },
    }
}

// OP_GroupUpdate full roster — eql-only (the legends variable-length format).
#[cfg(feature = "backend-eql")]
fn decode_group_roster(bytes: &[u8]) -> ffi::GroupRoster {
    match seq_backend_eql::parse_group_roster(bytes) {
        Ok(g) => ffi::GroupRoster {
            group_id: g.group_id,
            names: g.members.join("\n"),
            ok: true,
        },
        Err(_) => ffi::GroupRoster {
            group_id: 0,
            names: String::new(),
            ok: false,
        },
    }
}
#[cfg(not(feature = "backend-eql"))]
fn decode_group_roster(_bytes: &[u8]) -> ffi::GroupRoster {
    ffi::GroupRoster {
        group_id: 0,
        names: String::new(),
        ok: false,
    }
}

fn decode_group_member_list(bytes: &[u8]) -> ffi::GroupMemberList {
    match backend::group_member_list::parse_group_member_list(bytes) {
        Ok(g) => ffi::GroupMemberList {
            group_id: g.group_id,
            member_count: g.member_count,
            names: g.names.join("\n"),
            ok: true,
        },
        Err(_) => ffi::GroupMemberList {
            group_id: 0,
            member_count: 0,
            names: String::new(),
            ok: false,
        },
    }
}

fn decode_corpse_loc(bytes: &[u8]) -> ffi::CorpseLoc {
    match backend::parse_corpse_loc(bytes) {
        Ok(c) => ffi::CorpseLoc {
            spawn_id: c.spawn_id,
            x: c.x,
            y: c.y,
            z: c.z,
            ok: true,
        },
        Err(_) => ffi::CorpseLoc {
            spawn_id: 0,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            ok: false,
        },
    }
}

// Stage A+7

fn decode_door(bytes: &[u8]) -> ffi::Door {
    match backend::parse_door(bytes) {
        Ok(d) => ffi::Door {
            name: d.name,
            y: d.y,
            x: d.x,
            z: d.z,
            heading: d.heading,
            incline: d.incline,
            size: d.size,
            door_id: d.door_id,
            opentype: d.opentype,
            spawnstate: d.spawnstate,
            invertstate: d.invertstate,
            zone_point: d.zone_point,
            ok: true,
        },
        Err(_) => ffi::Door {
            name: String::new(),
            y: 0.0,
            x: 0.0,
            z: 0.0,
            heading: 0.0,
            incline: 0,
            size: 0,
            door_id: 0,
            opentype: 0,
            spawnstate: 0,
            invertstate: 0,
            zone_point: 0,
            ok: false,
        },
    }
}

fn decode_ground_spawn(bytes: &[u8]) -> ffi::GroundSpawn {
    match backend::parse_ground_spawn(bytes) {
        Ok(g) => ffi::GroundSpawn {
            drop_id: g.drop_id,
            id_file: g.id_file,
            heading: g.heading,
            y: g.y,
            x: g.x,
            z: g.z,
            bytes_consumed: g.bytes_consumed,
            ok: true,
        },
        Err(_) => ffi::GroundSpawn {
            drop_id: 0,
            id_file: String::new(),
            heading: 0.0,
            y: 0.0,
            x: 0.0,
            z: 0.0,
            bytes_consumed: 0,
            ok: false,
        },
    }
}

fn decode_zone_point(bytes: &[u8]) -> ffi::ZonePoint {
    match backend::parse_zone_point(bytes) {
        Ok(p) => ffi::ZonePoint {
            zone_trigger: p.zone_trigger,
            y: p.y,
            x: p.x,
            z: p.z,
            heading: p.heading,
            zone_id: p.zone_id,
            zone_instance: p.zone_instance,
            ok: true,
        },
        Err(_) => ffi::ZonePoint {
            zone_trigger: 0,
            y: 0.0,
            x: 0.0,
            z: 0.0,
            heading: 0.0,
            zone_id: 0,
            zone_instance: 0,
            ok: false,
        },
    }
}

fn decode_simple_message(bytes: &[u8]) -> ffi::SimpleMessage {
    match backend::parse_simple_message(bytes) {
        Ok(m) => ffi::SimpleMessage {
            message_format: m.message_format,
            message_color: m.message_color,
            ok: true,
        },
        Err(_) => ffi::SimpleMessage {
            message_format: 0,
            message_color: 0,
            ok: false,
        },
    }
}

fn formatted_message_err() -> ffi::FormattedMessage {
    ffi::FormattedMessage {
        message_format: 0,
        message_color: 0,
        spell_id: 0,
        msg_type: 0,
        spawn_id: 0,
        format_id: 0,
        args: Vec::new(),
        ok: false,
    }
}

// live/test: stock formattedMessageStruct (format id + chat colour); the EQL
// enrichment fields stay empty.
#[cfg(not(feature = "backend-eql"))]
fn decode_formatted_message(bytes: &[u8]) -> ffi::FormattedMessage {
    match backend::parse_formatted_message(bytes) {
        Ok(m) => ffi::FormattedMessage {
            message_format: m.message_format,
            message_color: m.message_color,
            spell_id: 0,
            msg_type: 0,
            spawn_id: 0,
            format_id: 0,
            args: Vec::new(),
            ok: true,
        },
        Err(_) => formatted_message_err(),
    }
}

// eql: OP_FormattedMessage carries a stock length-prefixed body —
// formatId@5, msgType/colour@9, length-prefixed args@13 (see the parser). The
// args are already positional (empty slots dropped, links cleaned); the daemon
// interpolates via EQStr::formatMessage(format_id, args). message_format/
// message_color mirror format_id/msg_color for stock symbol compatibility.
#[cfg(feature = "backend-eql")]
fn decode_formatted_message(bytes: &[u8]) -> ffi::FormattedMessage {
    match backend::parse_formatted_message(bytes) {
        Ok(m) => ffi::FormattedMessage {
            message_format: m.format_id,
            message_color: m.msg_color,
            spell_id: 0,
            msg_type: 0,
            spawn_id: 0,
            format_id: m.format_id,
            args: m.args,
            ok: true,
        },
        Err(_) => formatted_message_err(),
    }
}

fn decode_special_message(bytes: &[u8]) -> ffi::SpecialMessage {
    match backend::parse_special_message(bytes) {
        Ok(m) => ffi::SpecialMessage {
            message_color: m.message_color,
            target: m.target,
            source: m.source,
            message: m.message,
            ok: true,
        },
        Err(_) => ffi::SpecialMessage {
            message_color: 0,
            target: 0,
            source: String::new(),
            message: String::new(),
            ok: false,
        },
    }
}

fn decode_channel_message(bytes: &[u8]) -> ffi::ChannelMessage {
    match backend::parse_channel_message(bytes) {
        Ok(m) => ffi::ChannelMessage {
            sender: m.sender,
            target: m.target,
            language: m.language,
            chan_num: m.chan_num,
            skill_in_language: m.skill_in_language,
            message: m.message,
            ok: true,
        },
        Err(_) => ffi::ChannelMessage {
            sender: String::new(),
            target: String::new(),
            language: 0,
            chan_num: 0,
            skill_in_language: 0,
            message: String::new(),
            ok: false,
        },
    }
}

fn decode_player_profile(bytes: &[u8]) -> ffi::PlayerProfile {
    let parsed = backend::parse_player_profile(bytes);
    match parsed {
        Ok(p) => ffi::PlayerProfile {
            ok: true,
            bytes_consumed: p.bytes_consumed,
            checksum: p.checksum,
            gender: p.gender,
            race: p.race,
            class_: p.class_,
            class_mask: p.class_mask,
            stance: p.stance,
            invocation: p.invocation,
            level: p.level,
            level1: p.level1,
            bind0_zone_id: p.bind0_zone_id,
            bind0_x: p.bind0_x,
            bind0_y: p.bind0_y,
            bind0_z: p.bind0_z,
            bind0_heading: p.bind0_heading,
            deity: p.deity,
            intoxication: p.intoxication,
            points: p.points,
            mana: p.mana,
            cur_hp: p.cur_hp,
            str_: p.str_,
            sta: p.sta,
            cha: p.cha,
            dex: p.dex,
            int_: p.int_,
            agi: p.agi,
            wis: p.wis,
            aa_ids: p.aa_ids,
            aa_values: p.aa_values,
            skills: p.skills,
            disciplines: p.disciplines,
            recast_timers: p.recast_timers,
            spell_book: p.spell_book,
            mem_spells: p.mem_spells,
            spell_slot_refresh: p.spell_slot_refresh,
            buff_spell_ids: p.buff_spell_ids,
            buff_durations: p.buff_durations,
            platinum: p.platinum,
            gold: p.gold,
            silver: p.silver,
            copper: p.copper,
            platinum_cursor: p.platinum_cursor,
            gold_cursor: p.gold_cursor,
            silver_cursor: p.silver_cursor,
            copper_cursor: p.copper_cursor,
            aa_spent: p.aa_spent,
            aa_assigned: p.aa_assigned,
            aa_unspent: p.aa_unspent,
            endurance: p.endurance,
            exp_aa: p.exp_aa,
            name: p.name,
            last_name: p.last_name,
            birthday_time: p.birthday_time,
            account_create_date: p.account_create_date,
            last_save_time: p.last_save_time,
            time_played_min: p.time_played_min,
            expansions: p.expansions,
            languages: p.languages,
            zone_id: p.zone_id,
            zone_instance: p.zone_instance,
            x: p.x,
            y: p.y,
            z: p.z,
            heading: p.heading,
            stand_state: p.stand_state,
            anon: p.anon,
            guild_id: p.guild_id,
            guild_server_id: p.guild_server_id,
            platinum_inventory: p.platinum_inventory,
            gold_inventory: p.gold_inventory,
            silver_inventory: p.silver_inventory,
            copper_inventory: p.copper_inventory,
            platinum_bank: p.platinum_bank,
            gold_bank: p.gold_bank,
            silver_bank: p.silver_bank,
            copper_bank: p.copper_bank,
            platinum_shared: p.platinum_shared,
            career_tribute: p.career_tribute,
            current_tribute: p.current_tribute,
            current_rad_crystals: p.current_rad_crystals,
            career_rad_crystals: p.career_rad_crystals,
            current_ebon_crystals: p.current_ebon_crystals,
            career_ebon_crystals: p.career_ebon_crystals,
            autosplit: p.autosplit,
            ldon_guk_points: p.ldon_guk_points,
            ldon_mir_points: p.ldon_mir_points,
            ldon_mmc_points: p.ldon_mmc_points,
            ldon_ruj_points: p.ldon_ruj_points,
            ldon_tak_points: p.ldon_tak_points,
            ldon_avail_points: p.ldon_avail_points,
        },
        Err(_) => ffi::PlayerProfile {
            ok: false,
            bytes_consumed: 0,
            checksum: 0,
            gender: 0,
            race: 0,
            class_: 0,
            class_mask: 0,
            stance: 0,
            invocation: 0,
            level: 0,
            level1: 0,
            bind0_zone_id: 0,
            bind0_x: 0.0,
            bind0_y: 0.0,
            bind0_z: 0.0,
            bind0_heading: 0.0,
            deity: 0,
            intoxication: 0,
            points: 0,
            mana: 0,
            cur_hp: 0,
            str_: 0,
            sta: 0,
            cha: 0,
            dex: 0,
            int_: 0,
            agi: 0,
            wis: 0,
            aa_ids: Vec::new(),
            aa_values: Vec::new(),
            skills: Vec::new(),
            disciplines: Vec::new(),
            recast_timers: Vec::new(),
            spell_book: Vec::new(),
            mem_spells: Vec::new(),
            spell_slot_refresh: Vec::new(),
            buff_spell_ids: Vec::new(),
            buff_durations: Vec::new(),
            platinum: 0,
            gold: 0,
            silver: 0,
            copper: 0,
            platinum_cursor: 0,
            gold_cursor: 0,
            silver_cursor: 0,
            copper_cursor: 0,
            aa_spent: 0,
            aa_assigned: 0,
            aa_unspent: 0,
            endurance: 0,
            exp_aa: 0,
            name: String::new(),
            last_name: String::new(),
            birthday_time: 0,
            account_create_date: 0,
            last_save_time: 0,
            time_played_min: 0,
            expansions: 0,
            languages: Vec::new(),
            zone_id: 0,
            zone_instance: 0,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            heading: 0.0,
            stand_state: 0,
            anon: 0,
            guild_id: 0,
            guild_server_id: 0,
            platinum_inventory: 0,
            gold_inventory: 0,
            silver_inventory: 0,
            copper_inventory: 0,
            platinum_bank: 0,
            gold_bank: 0,
            silver_bank: 0,
            copper_bank: 0,
            platinum_shared: 0,
            career_tribute: 0,
            current_tribute: 0,
            current_rad_crystals: 0,
            career_rad_crystals: 0,
            current_ebon_crystals: 0,
            career_ebon_crystals: 0,
            autosplit: 0,
            ldon_guk_points: 0,
            ldon_mir_points: 0,
            ldon_mmc_points: 0,
            ldon_ruj_points: 0,
            ldon_tak_points: 0,
            ldon_avail_points: 0,
        },
    }
}

fn decode_new_zone(bytes: &[u8]) -> ffi::NewZone {
    let parsed = backend::parse_new_zone(bytes);
    match parsed {
        Ok(z) => ffi::NewZone {
            short_name: z.short_name,
            long_name: z.long_name,
            zonefile: z.zonefile,
            zone_exp_multiplier: z.zone_exp_multiplier,
            safe_y: z.safe_y,
            safe_x: z.safe_x,
            safe_z: z.safe_z,
            zone_id: z.zone_id,
            ok: true,
        },
        Err(_) => ffi::NewZone {
            short_name: String::new(),
            long_name: String::new(),
            zonefile: String::new(),
            zone_exp_multiplier: 0.0,
            safe_y: 0.0,
            safe_x: 0.0,
            safe_z: 0.0,
            zone_id: 0,
            ok: false,
        },
    }
}

// Stage A+8

fn decode_player_self_pos(bytes: &[u8]) -> ffi::PlayerSelfPos {
    let parsed = backend::parse_player_self_pos(bytes);
    match parsed {
        Ok(p) => ffi::PlayerSelfPos {
            spawn_id: p.spawn_id,
            x: p.x,
            y: p.y,
            z: p.z,
            delta_x: p.delta_x,
            delta_y: p.delta_y,
            delta_z: p.delta_z,
            heading: p.heading,
            delta_heading: p.delta_heading,
            animation: p.animation,
            pitch: p.pitch,
            ok: true,
        },
        Err(_) => ffi::PlayerSelfPos {
            spawn_id: 0,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            delta_x: 0.0,
            delta_y: 0.0,
            delta_z: 0.0,
            heading: 0,
            delta_heading: 0,
            animation: 0,
            pitch: 0,
            ok: false,
        },
    }
}

fn decode_player_spawn_pos(bytes: &[u8]) -> ffi::PlayerSpawnPos {
    match backend::parse_player_spawn_pos(bytes) {
        Ok(p) => ffi::PlayerSpawnPos {
            spawn_id: p.spawn_id,
            spawn_id2: p.spawn_id2,
            x: p.x,
            y: p.y,
            z: p.z,
            delta_x: p.delta_x,
            delta_y: p.delta_y,
            delta_z: p.delta_z,
            heading: p.heading,
            delta_heading: p.delta_heading,
            animation: p.animation,
            pitch: p.pitch,
            ok: true,
        },
        Err(_) => ffi::PlayerSpawnPos {
            spawn_id: 0,
            spawn_id2: 0,
            x: 0,
            y: 0,
            z: 0,
            delta_x: 0,
            delta_y: 0,
            delta_z: 0,
            heading: 0,
            delta_heading: 0,
            animation: 0,
            pitch: 0,
            ok: false,
        },
    }
}

fn decode_npc_move_update(bytes: &[u8]) -> ffi::NpcMove {
    match backend::parse_npc_move_update(bytes) {
        Ok(n) => ffi::NpcMove {
            spawn_id: n.spawn_id,
            x: n.x,
            y: n.y,
            z: n.z,
            heading: n.heading,
            delta_x: n.delta_x,
            delta_y: n.delta_y,
            delta_z: n.delta_z,
            delta_heading: n.delta_heading,
            animation: n.animation,
            ok: true,
        },
        Err(_) => ffi::NpcMove {
            spawn_id: 0,
            x: 0,
            y: 0,
            z: 0,
            heading: 0,
            delta_x: 0,
            delta_y: 0,
            delta_z: 0,
            delta_heading: 0,
            animation: 0,
            ok: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_zero_payload() {
        let r = decode_mob_update(&[0u8; 14]);
        assert!(r.ok);
        assert_eq!(r.spawn_id, 0);
        assert_eq!(r.x, 0);
    }

    #[test]
    fn bad_length_returns_ok_false() {
        let r = decode_mob_update(&[0u8; 13]);
        assert!(!r.ok);
    }

    #[test]
    fn delete_spawn_roundtrip() {
        let bytes = [0xEF, 0xBE, 0xAD, 0xDE];
        let r = decode_delete_spawn(&bytes);
        assert!(r.ok);
        assert_eq!(r.spawn_id, 0xDEADBEEF);
    }

    #[test]
    fn delete_spawn_bad_length_returns_ok_false() {
        let r = decode_delete_spawn(&[0u8; 3]);
        assert!(!r.ok);
    }
}

#[cfg(test)]
mod session_tests;
