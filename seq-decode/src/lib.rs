//! Pure parsers for EverQuest packet payloads.
//!
//! No I/O, no global state, no Qt. Each module exposes a `parse_*`
//! function that turns a `&[u8]` payload into a typed struct (or a
//! `ParseError`). Higher layers (FFI bridge, replay tools, the
//! eventual standalone daemon) compose these.
//!
//! Stage A: `OP_MobUpdate`. Stage A+1: `OP_DeleteSpawn`.
//! Stage A+2: spawn (variable-length payload from `OP_ZoneEntry`
//! server-direction; mirrors `SpawnShell::fillSpawnStruct`).
//! Stage A+3: small fixed-size batch — OP_RemoveSpawn, OP_HPUpdate,
//! OP_MobHealth, OP_SpawnAppearance, OP_ExpUpdate, OP_LevelUpdate,
//! OP_SkillUpdate.
//! Stage A+4: more small fixed-size — OP_ManaChange, OP_Stamina,
//! OP_EndUpdate, OP_Consider, OP_SpawnRename, OP_TargetMouse,
//! OP_Death.
//! Stage A+5: OP_ClickObject, OP_Illusion, OP_Buff, OP_Action2.
//! Stage A+6: OP_WearChange, OP_ZoneChange, OP_DzInfo,
//! OP_DzSwitchInfo, OP_CastSpell, OP_Action (two-variant),
//! OP_GroupDisband / OP_GroupDisband2, OP_GroupFollow,
//! OP_CorpseLocResponse.
//! Stage A+7: OP_SpawnDoor (per-element), OP_GroundSpawn
//! (NetStream-style variable text).
//! Stage A+8: OP_ClientUpdate (playerSelfPosStruct 42b +
//! playerSpawnPosStruct 28b — bitfield-laden), OP_NpcMoveUpdate
//! (variable-length 13..24b BitStream / sign-magnitude packing).

// Backend-selected struct bindings. Parsers reference `crate::eqstructs::<name>`;
// this aliases the generated crate for the active backend. seq-decode is the
// shared Live decode layer: `backend-live` and `backend-test` pick the matching
// struct-mirror crate. eql reuses `backend-live` (its wire matches Live for the
// ~38 shared opcodes) and layers `seq-backend-eql` on top for the 5 diverged
// ones — so seq-decode itself has no `backend-eql` feature.
#[cfg(not(any(feature = "backend-live", feature = "backend-test")))]
compile_error!("seq-decode: enable exactly one backend feature (default: backend-live)");
#[cfg(feature = "backend-live")]
pub(crate) use seq_structs_live as eqstructs;
#[cfg(feature = "backend-test")]
pub(crate) use seq_structs_test as eqstructs;

pub mod action;
pub mod action2;
pub mod action_alt;
pub mod alt_exp_update;
pub mod buff;
pub mod channel_message;
pub mod click_object;
pub mod client_target;
pub mod consider;
pub mod corpse_loc;
pub mod cursor;
pub mod death;
pub mod delete_spawn;
pub mod dz_info;
pub mod dz_switch_info;
pub mod end_update;
pub mod exp_update;
pub mod formatted_message;
pub mod ground_spawn;
pub mod group_disband;
pub mod group_follow;
pub mod group_member_list;
pub mod guild_expanded_info;
pub mod guild_in_zone;
pub mod guild_member_update;
pub mod guild_motd;
pub mod guild_roster;
pub mod hp_update;
pub mod illusion;
pub mod item_packet;
pub mod level_update;
pub mod mana_change;
pub mod mob_health;
pub mod mob_update;
pub mod new_zone;
pub mod npc_move_update;
pub mod player_profile;
pub mod player_self_pos;
pub mod player_spawn_pos;
pub mod remove_spawn;
pub mod simple_message;
pub mod skill_update;
pub mod spawn;
pub mod spawn_appearance;
pub mod spawn_door;
pub mod spawn_rename;
pub mod special_message;
pub mod stamina;
pub mod start_cast;
pub mod time_of_day;
pub mod wear_change;
pub mod zone_change;
pub mod zone_point;
pub mod zone_server_info;

pub use action::{parse_action, Action, ActionError};
pub use action2::{parse_action2, Action2, Action2Error};
pub use action_alt::{parse_action_alt, ActionAlt, ActionAltError};
pub use buff::{parse_buff, Buff, BuffError};
pub use channel_message::{parse_channel_message, ChannelMessage, ChannelMessageError};
pub use click_object::{parse_click_object, ClickObject, ClickObjectError};
pub use client_target::{parse_client_target, ClientTarget, ClientTargetError};
pub use consider::{parse_consider, Consider, ConsiderError};
pub use corpse_loc::{parse_corpse_loc, CorpseLoc, CorpseLocError};
pub use death::{parse_death, Death, DeathError};
pub use delete_spawn::{
    parse_delete_spawn, DeleteSpawn, DeleteSpawnError, PAYLOAD_LEN as DELETE_SPAWN_LEN,
};
pub use dz_info::{parse_dz_info, DzInfo, DzInfoError};
pub use dz_switch_info::{parse_dz_switch_info, DzSwitch, DzSwitchError};
pub use end_update::{parse_end_update, EndUpdate, EndUpdateError};
pub use exp_update::{parse_exp_update, ExpUpdate, ExpUpdateError};
pub use formatted_message::{parse_formatted_message, FormattedMessage, FormattedMessageError};
pub use ground_spawn::{parse_ground_spawn, GroundSpawn, GroundSpawnError};
pub use group_disband::{parse_group_disband, GroupDisband, GroupDisbandError};
pub use group_follow::{parse_group_follow, GroupFollow, GroupFollowError};
pub use hp_update::{parse_hp_update, HpUpdate, HpUpdateError};
pub use illusion::{parse_illusion, Illusion, IllusionError};
pub use level_update::{parse_level_update, LevelUpdate, LevelUpdateError};
pub use mana_change::{parse_mana_change, ManaChange, ManaChangeError};
pub use mob_health::{parse_mob_health, MobHealth, MobHealthError};
pub use mob_update::{parse_mob_update, MobUpdate, ParseError, PAYLOAD_LEN as MOB_UPDATE_LEN};
pub use new_zone::{parse_new_zone, NewZone, NewZoneError};
pub use npc_move_update::{parse_npc_move_update, NpcMoveUpdate, NpcMoveUpdateError};
pub use player_profile::{parse_player_profile, PlayerProfile, PlayerProfileError};
pub use player_self_pos::{parse_player_self_pos, PlayerSelfPos, PlayerSelfPosError};
pub use player_spawn_pos::{parse_player_spawn_pos, PlayerSpawnPos, PlayerSpawnPosError};
pub use remove_spawn::{parse_remove_spawn, RemoveSpawn, RemoveSpawnError};
pub use simple_message::{parse_simple_message, SimpleMessage, SimpleMessageError};
pub use skill_update::{parse_skill_update, SkillUpdate, SkillUpdateError};
pub use spawn::{parse_spawn, Spawn, SpawnError};
pub use spawn_appearance::{parse_spawn_appearance, SpawnAppearance, SpawnAppearanceError};
pub use spawn_door::{parse_door, Door, DoorError};
pub use spawn_rename::{parse_spawn_rename, SpawnRename, SpawnRenameError};
pub use special_message::{parse_special_message, SpecialMessage, SpecialMessageError};
pub use stamina::{parse_stamina, Stamina, StaminaError};
pub use start_cast::{parse_start_cast, StartCast, StartCastError};
pub use wear_change::{parse_wear_change, WearChange, WearChangeError};
pub use zone_change::{parse_zone_change, ZoneChange, ZoneChangeError};
pub use zone_point::{parse_zone_point, ZonePoint, ZonePointError};
pub use zone_server_info::{parse_zone_server_info, ZoneServerInfo, ZoneServerInfoError};

/// Decode a NUL-padded byte buffer (a wire-format C-string field) into
/// an owned `String`. Bytes after the first NUL are dropped; invalid
/// UTF-8 is replaced with U+FFFD. Used by parsers whose payload mirrors
/// `char[N]` fields on the C side.
pub(crate) fn cstr_field(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}
