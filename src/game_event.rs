use crate::{csgo_proto, RawValue, UserId};

macro_rules! define_event {
    ($name:ident, $target:path $(, ($field:ident, $field_ty:ty))*) => {
        #[derive(Debug)]
        #[allow(dead_code)]
        pub struct $name {
            $(pub $field: Option<$field_ty>,)*
            pub remaining: ::std::collections::HashMap<String, crate::csgo_proto::c_msg_source1_legacy_game_event::KeyT>,
        }

        impl $name {
            #[allow(unused_mut)]
            fn parse(keys: &[crate::csgo_proto::csvc_msg_game_event_list::KeyT], event: crate::csgo_proto::CMsgSource1LegacyGameEvent) -> Result<GameEvent, ParseGameEventError> {

                $(let mut $field: Option<RawValue> = None;)*
                let mut remaining = std::collections::HashMap::new();

                for (k, f) in keys.iter().zip(event.keys.into_iter()) {
                    let name = k.name();
                    $(
                    if name == stringify!($field) {
                        $field = Some(f.try_into().ok()).flatten();
                        continue;
                    }
                    )*

                    remaining.insert(name.to_owned(), f);
                }

                let value = $name {
                    $($field: $field.map(|f| f.try_into().ok()).flatten(),)*
                    remaining,
                };

                Ok($target(value))
            }
        }
    };
}

define_event!(HltvVersionInfo, GameEvent::HltvVersionInfo);

define_event!(
    ItemEquip,
    GameEvent::ItemEquip,
    (userid, UserId),
    (hassilencer, bool),
    (hastracers, bool),
    (item, String),
    (issilenced, bool),
    (canzoom, bool),
    (ispainted, bool),
    (weptype, i32),
    (defindex, i32)
);
define_event!(
    ItemPickup,
    GameEvent::ItemPickup,
    (userid, UserId),
    (item, RawValue),
    (silent, RawValue),
    (defindex, RawValue)
);

define_event!(
    WeaponReload,
    GameEvent::WeaponReload,
    (userid, UserId),
    (userid_pawn, RawValue)
);
define_event!(
    WeaponZoom,
    GameEvent::WeaponZoom,
    (userid, UserId),
    (userid_pawn, RawValue)
);
define_event!(
    WeaponFire,
    GameEvent::WeaponFire,
    (userid, UserId),
    (weapon, RawValue),
    (silenced, RawValue),
    (userid_pawn, RawValue)
);

define_event!(SmokeGrenadeDetonate, GameEvent::SmokeGrenadeDetonate);
define_event!(SmokeGrenadeExpired, GameEvent::SmokeGrenadeExpired);
define_event!(HEGrenadeDetonate, GameEvent::HEGrenadeDetonate);
define_event!(InfernoStartBurn, GameEvent::InfernoStartBurn);
define_event!(InfernoExpire, GameEvent::InfernoExpire);
define_event!(FlashbangDetonate, GameEvent::FlashbangDetonate);
define_event!(DecoyStarted, GameEvent::DecoyStarted);
define_event!(DecoyDetonate, GameEvent::DecoyDetonate);

define_event!(
    PlayerConnect,
    GameEvent::PlayerConnect,
    (address, RawValue),
    (bot, RawValue),
    (name, RawValue),
    (userid, i32),
    (networkid, RawValue),
    (xuid, RawValue)
);
define_event!(
    PlayerConnectFull,
    GameEvent::PlayerConnectFull,
    (userid, UserId)
);
define_event!(
    PlayerDisconnect,
    GameEvent::PlayerDisconnect,
    (userid, UserId),
    (reason, RawValue),
    (name, RawValue),
    (networkid, RawValue),
    (xuid, RawValue)
);
define_event!(
    PlayerFootstep,
    GameEvent::PlayerFootstep,
    (userid, UserId),
    (userid_pawn, RawValue)
);
define_event!(PlayerJump, GameEvent::PlayerJump, (userid, i32));
define_event!(
    PlayerHurt,
    GameEvent::PlayerHurt,
    (userid, UserId),
    (attacker, UserId),
    (health, RawValue),
    (armor, RawValue),
    (weapon, String),
    (dmg_health, RawValue),
    (dmg_armor, RawValue),
    (hitgroup, RawValue),
    (userid_pawn, RawValue),
    (attacker_pawn, RawValue)
);
define_event!(
    PlayerDeath,
    GameEvent::PlayerDeath,
    (userid, UserId),
    (attacker, UserId),
    (assister, UserId),
    (assistedflash, bool),
    (weapon, String),
    (weapon_itemid, String),
    (weapon_fauxitemid, String),
    (weapon_originalowner_xuid, String),
    (headshot, bool),
    (dominated, RawValue),
    (revenge, RawValue),
    (wipe, RawValue),
    (penetrated, RawValue),
    (noreplay, bool),
    (noscope, bool),
    (thrusmoke, bool),
    (attackerblind, bool),
    (distance, RawValue),
    (userid_pawn, RawValue),
    (attacker_pawn, RawValue),
    (assister_pawn, RawValue),
    (dmg_health, RawValue),
    (dmg_armor, RawValue),
    (hitgroup, RawValue),
    (attackerinair, bool)
);
define_event!(
    PlayerSpawn,
    GameEvent::PlayerSpawn,
    (userid, UserId),
    (inrestart, RawValue),
    (userid_pawn, RawValue)
);
define_event!(
    PlayerBlind,
    GameEvent::PlayerBlind,
    (userid, UserId),
    (attacker, RawValue),
    (entityid, RawValue),
    (blind_duration, RawValue)
);
define_event!(
    PlayerTeam,
    GameEvent::PlayerTeam,
    (userid, UserId),
    (team, RawValue),
    (oldteam, RawValue),
    (disconnect, RawValue),
    (silent, RawValue),
    (isbot, RawValue),
    (userid_pawn, RawValue)
);

define_event!(BulletDamage, GameEvent::BulletDamage);

define_event!(OtherDeath, GameEvent::OtherDeath);

define_event!(BombPickup, GameEvent::BombPickup);
define_event!(BombDropped, GameEvent::BombDropped);
define_event!(BombBeginPlant, GameEvent::BombBeginPlant);
define_event!(BombPlanted, GameEvent::BombPlanted);
define_event!(BombExploded, GameEvent::BombExploded);
define_event!(BombBeginDefuse, GameEvent::BombBeginDefuse);
define_event!(BombDefused, GameEvent::BombDefused);

define_event!(BeginNewMatch, GameEvent::BeginNewMatch);
define_event!(RoundAnnounceMatchStart, GameEvent::RoundAnnounceMatchStart);
define_event!(RoundFreezeEnd, GameEvent::RoundFreezeEnd);
define_event!(RoundPreStart, GameEvent::RoundPreStart);
define_event!(RoundPostStart, GameEvent::RoundPostStart);
define_event!(RoundOfficiallyEnded, GameEvent::RoundOfficiallyEnded);
define_event!(RoundStartBeep, GameEvent::RoundStartBeep);
define_event!(RoundAnnounceMatchpoint, GameEvent::RoundAnnounceMatchpoint);
define_event!(RoundPreRestart, GameEvent::RoundPreRestart);
define_event!(RoundTimeWarning, GameEvent::RoundTimeWarning);
define_event!(RoundFinalBeep, GameEvent::RoundFinalBeep);
define_event!(BuyTimeEnded, GameEvent::BuyTimeEnded);
define_event!(
    RoundAnnounceLastRoundHalf,
    GameEvent::RoundAnnounceLastRoundHalf
);
define_event!(AnnouncePhaseEnd, GameEvent::AnnouncePhaseEnd);

define_event!(WinPanelMatch, GameEvent::WinPanelMatch);

type ParseFn = fn(
    keys: &[csgo_proto::csvc_msg_game_event_list::KeyT],
    event: csgo_proto::CMsgSource1LegacyGameEvent,
) -> Result<GameEvent, ParseGameEventError>;

#[derive(Debug)]
#[allow(dead_code)]
pub enum GameEvent {
    HltvVersionInfo(HltvVersionInfo),
    //
    ItemEquip(ItemEquip),
    ItemPickup(ItemPickup),
    //
    WeaponReload(WeaponReload),
    WeaponZoom(WeaponZoom),
    WeaponFire(WeaponFire),
    //
    SmokeGrenadeDetonate(SmokeGrenadeDetonate),
    SmokeGrenadeExpired(SmokeGrenadeExpired),
    HEGrenadeDetonate(HEGrenadeDetonate),
    InfernoStartBurn(InfernoStartBurn),
    InfernoExpire(InfernoExpire),
    FlashbangDetonate(FlashbangDetonate),
    DecoyStarted(DecoyStarted),
    DecoyDetonate(DecoyDetonate),
    //
    PlayerConnect(PlayerConnect),
    PlayerConnectFull(PlayerConnectFull),
    PlayerDisconnect(PlayerDisconnect),
    PlayerFootstep(PlayerFootstep),
    PlayerJump(PlayerJump),
    PlayerHurt(PlayerHurt),
    PlayerDeath(PlayerDeath),
    PlayerSpawn(PlayerSpawn),
    PlayerBlind(PlayerBlind),
    PlayerTeam(PlayerTeam),
    //
    BulletDamage(BulletDamage),
    //
    OtherDeath(OtherDeath),
    //
    BombPickup(BombPickup),
    BombDropped(BombDropped),
    BombBeginPlant(BombBeginPlant),
    BombPlanted(BombPlanted),
    BombExploded(BombExploded),
    BombBeginDefuse(BombBeginDefuse),
    BombDefused(BombDefused),
    //
    BeginNewMatch(BeginNewMatch),
    RoundAnnounceMatchStart(RoundAnnounceMatchStart),
    RoundFreezeEnd(RoundFreezeEnd),
    RoundPreStart(RoundPreStart),
    RoundPostStart(RoundPostStart),
    RoundOfficiallyEnded(RoundOfficiallyEnded),
    RoundStartBeep(RoundStartBeep),
    RoundAnnounceMatchpoint(RoundAnnounceMatchpoint),
    RoundPreRestart(RoundPreRestart),
    RoundTimeWarning(RoundTimeWarning),
    RoundFinalBeep(RoundFinalBeep),
    BuyTimeEnded(BuyTimeEnded),
    RoundAnnounceLastRoundHalf(RoundAnnounceLastRoundHalf),
    AnnouncePhaseEnd(AnnouncePhaseEnd),

    WinPanelMatch(WinPanelMatch),
}

#[derive(Debug)]
pub enum ParseGameEventError {
    MismatchedKeysFields,
}

pub static EVENT_PARSERS: phf::Map<&'static str, GameEventParser> = phf::phf_map! {
    "hltv_versioninfo" => GameEventParser::new(HltvVersionInfo::parse),

    "item_equip" => GameEventParser::new(ItemEquip::parse),
    "item_pickup" => GameEventParser::new(ItemPickup::parse),

    "weapon_reload" => GameEventParser::new(WeaponReload::parse),
    "weapon_zoom" => GameEventParser::new(WeaponZoom::parse),
    "weapon_fire" => GameEventParser::new(WeaponFire::parse),

    "smokegrenade_detonate" => GameEventParser::new(SmokeGrenadeDetonate::parse),
    "smokegrenade_expired" => GameEventParser::new(SmokeGrenadeExpired::parse),
    "hegrenade_detonate" => GameEventParser::new(HEGrenadeDetonate::parse),
    "inferno_startburn" => GameEventParser::new(InfernoStartBurn::parse),
    "inferno_expire" => GameEventParser::new(InfernoExpire::parse),
    "flashbang_detonate" => GameEventParser::new(FlashbangDetonate::parse),
    "decoy_started" => GameEventParser::new(DecoyStarted::parse),
    "decoy_detonate" => GameEventParser::new(DecoyDetonate::parse),

    "player_connect" => GameEventParser::new(PlayerConnect::parse),
    "player_connect_full" => GameEventParser::new(PlayerConnectFull::parse),
    "player_disconnect" => GameEventParser::new(PlayerDisconnect::parse),
    "player_footstep" => GameEventParser::new(PlayerFootstep::parse),
    "player_jump" => GameEventParser::new(PlayerJump::parse),
    "player_hurt" => GameEventParser::new(PlayerHurt::parse),
    "player_death" => GameEventParser::new(PlayerDeath::parse),
    "player_spawn" => GameEventParser::new(PlayerSpawn::parse),
    "player_blind" => GameEventParser::new(PlayerBlind::parse),
    "player_team" => GameEventParser::new(PlayerTeam::parse),

    "bullet_damage" => GameEventParser::new(BulletDamage::parse),

    "other_death" => GameEventParser::new(OtherDeath::parse),

    "bomb_pickup" => GameEventParser::new(BombPickup::parse),
    "bomb_dropped" => GameEventParser::new(BombDropped::parse),
    "bomb_beginplant" => GameEventParser::new(BombBeginPlant::parse),
    "bomb_planted" => GameEventParser::new(BombPlanted::parse),
    "bomb_exploded" => GameEventParser::new(BombExploded::parse),
    "bomb_begindefuse" => GameEventParser::new(BombBeginDefuse::parse),
    "bomb_defused" => GameEventParser::new(BombDefused::parse),

    "begin_new_match" => GameEventParser::new(BeginNewMatch::parse),
    "round_announce_match_start" => GameEventParser::new(RoundAnnounceMatchStart::parse),
    "round_freeze_end" => GameEventParser::new(RoundFreezeEnd::parse),
    "round_prestart" => GameEventParser::new(RoundPreStart::parse),
    "round_poststart" => GameEventParser::new(RoundPostStart::parse),
    "round_officially_ended" => GameEventParser::new(RoundOfficiallyEnded::parse),
    "cs_round_start_beep" => GameEventParser::new(RoundStartBeep::parse),
    "round_announce_match_point" => GameEventParser::new(RoundAnnounceMatchpoint::parse),
    "cs_pre_restart" => GameEventParser::new(RoundPreRestart::parse),
    "round_time_warning" => GameEventParser::new(RoundTimeWarning::parse),
    "cs_round_final_beep" => GameEventParser::new(RoundFinalBeep::parse),
    "buytime_ended" => GameEventParser::new(BuyTimeEnded::parse),
    "round_announce_last_round_half" => GameEventParser::new(RoundAnnounceLastRoundHalf::parse),
    "announce_phase_end" => GameEventParser::new(AnnouncePhaseEnd::parse),

    "cs_win_panel_match" => GameEventParser::new(WinPanelMatch::parse),
};

pub struct GameEventParser {
    inner: fn(
        keys: &[csgo_proto::csvc_msg_game_event_list::KeyT],
        event: csgo_proto::CMsgSource1LegacyGameEvent,
    ) -> Result<GameEvent, ParseGameEventError>,
}

impl GameEventParser {
    pub const fn new(func: ParseFn) -> Self {
        Self { inner: func }
    }

    pub fn parse(
        &self,
        keys: &[csgo_proto::csvc_msg_game_event_list::KeyT],
        event: csgo_proto::CMsgSource1LegacyGameEvent,
    ) -> Result<GameEvent, ParseGameEventError> {
        if keys.len() != event.keys.len() {
            return Err(ParseGameEventError::MismatchedKeysFields);
        }

        (self.inner)(keys, event)
    }
}
