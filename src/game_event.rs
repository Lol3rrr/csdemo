use crate::csgo_proto;

#[derive(Debug)]
pub enum RawValue {
    String(String),
    F32(f32),
    I32(i32),
    Bool(bool),
    U64(u64),
}

impl TryFrom<crate::csgo_proto::c_msg_source1_legacy_game_event::KeyT> for RawValue {
    type Error = ();

    fn try_from(value: crate::csgo_proto::c_msg_source1_legacy_game_event::KeyT) -> Result<Self, Self::Error> {
        match value.r#type() {
            1 if value.val_string.is_some() => Ok(Self::String(value.val_string.unwrap())),
            2 if value.val_float.is_some() => Ok(Self::F32(value.val_float.unwrap())),
            3 if value.val_long.is_some() => Ok(Self::I32(value.val_long.unwrap())),
            4 if value.val_short.is_some() => Ok(Self::I32(value.val_short.unwrap() as i32)),
            5 if value.val_byte.is_some() => Ok(Self::I32(value.val_byte.unwrap() as i32)),
            6 if value.val_bool.is_some() => Ok(Self::Bool(value.val_bool.unwrap())),
            7 if value.val_uint64.is_some() => Ok(Self::U64(value.val_uint64.unwrap())),
            8 if value.val_long.is_some() => Ok(Self::I32(value.val_long.unwrap())),
            9 if value.val_short.is_some() => Ok(Self::I32(value.val_short.unwrap() as i32)),
            _ => Err(()),
        }
    }
}

impl TryFrom<RawValue> for i32 {
    type Error = ();
    fn try_from(value: RawValue) -> Result<Self, Self::Error> {
        match value {
            RawValue::I32(v) => Ok(v),
            _ => Err(()),
        }
    }
}
impl TryFrom<RawValue> for bool {
    type Error = ();
    fn try_from(value: RawValue) -> Result<Self, Self::Error> {
        match value {
            RawValue::Bool(v) => Ok(v),
            _ => Err(()),
        }
    }
}
impl TryFrom<RawValue> for String {
    type Error = ();
    fn try_from(value: RawValue) -> Result<Self, Self::Error> {
        match value {
            RawValue::String(v) => Ok(v),
            _ => Err(()),
        }
    }
}

macro_rules! define_event {
    ($name:ident, $target:path $(, ($field:ident, $field_ty:ty))*) => {
        #[derive(Debug)]
        #[allow(dead_code)]
        pub struct $name {
            $($field: Option<$field_ty>,)*
            remaining: ::std::collections::HashMap<String, crate::csgo_proto::c_msg_source1_legacy_game_event::KeyT>,
        }

        impl $name {
            #[allow(unused_mut)]
            fn parse(keys: &[crate::csgo_proto::csvc_msg_game_event_list::KeyT], event: crate::csgo_proto::CMsgSource1LegacyGameEvent) -> Result<GameEvent, ParseGameEventError> {
                let mut fields: ::std::collections::HashMap<_,_> = keys.iter().zip(event.keys.into_iter()).map(|(k, f)| {
                    (k.name().to_owned(), f)
                }).collect();

                $(let $field: Option<RawValue> = fields.remove(stringify!($field)).map(|f| f.try_into().ok()).flatten();)*

                let value = $name {
                    $($field: $field.map(|f| f.try_into().ok()).flatten(),)*
                    remaining: fields,
                };

                Ok($target(value))
            }
        }
    };
}

define_event!(HltvVersionInfo, GameEvent::HltvVersionInfo);

define_event!(ItemEquip, GameEvent::ItemEquip, (userid, i32), (hassilencer, bool), (hastracers, bool), (item, String), (issilenced, bool), (canzoom, bool), (ispainted, bool), (weptype, i32), (defindex, i32));
define_event!(ItemPickup, GameEvent::ItemPickup, (userid, RawValue), (item, RawValue), (silent, RawValue), (defindex, RawValue));

define_event!(WeaponReload, GameEvent::WeaponReload, (userid, RawValue), (userid_pawn, RawValue));
define_event!(WeaponZoom, GameEvent::WeaponZoom, (userid, RawValue), (userid_pawn, RawValue));
define_event!(WeaponFire, GameEvent::WeaponFire, (userid, RawValue), (weapon, RawValue), (silenced, RawValue), (userid_pawn, RawValue));

define_event!(SmokeGrenadeDetonate, GameEvent::SmokeGrenadeDetonate);
define_event!(SmokeGrenadeExpired, GameEvent::SmokeGrenadeExpired);
define_event!(HEGrenadeDetonate, GameEvent::HEGrenadeDetonate);
define_event!(InfernoStartBurn, GameEvent::InfernoStartBurn);
define_event!(InfernoExpire, GameEvent::InfernoExpire);
define_event!(FlashbangDetonate, GameEvent::FlashbangDetonate);
define_event!(DecoyStarted, GameEvent::DecoyStarted);
define_event!(DecoyDetonate, GameEvent::DecoyDetonate);

define_event!(PlayerConnect, GameEvent::PlayerConnect, (address, RawValue), (bot, RawValue), (name, RawValue), (userid, RawValue), (networkid, RawValue), (xuid, RawValue));
define_event!(PlayerConnectFull, GameEvent::PlayerConnectFull, (userid, RawValue));
define_event!(PlayerDisconnect, GameEvent::PlayerDisconnect, (userid, RawValue), (reason, RawValue), (name, RawValue), (networkid, RawValue), (xuid, RawValue));
define_event!(PlayerFootstep, GameEvent::PlayerFootstep);
define_event!(PlayerJump, GameEvent::PlayerJump);
define_event!(PlayerHurt, GameEvent::PlayerHurt);
define_event!(PlayerDeath, GameEvent::PlayerDeath);
define_event!(PlayerSpawn, GameEvent::PlayerSpawn);
define_event!(PlayerBlind, GameEvent::PlayerBlind);
define_event!(PlayerTeam, GameEvent::PlayerTeam, (userid, RawValue), (team, RawValue), (oldteam, RawValue), (disconnect, RawValue), (silent, RawValue), (isbot, RawValue), (userid_pawn, RawValue));

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
define_event!(RoundAnnounceLastRoundHalf, GameEvent::RoundAnnounceLastRoundHalf);
define_event!(AnnouncePhaseEnd, GameEvent::AnnouncePhaseEnd);

define_event!(WinPanelMatch, GameEvent::WinPanelMatch);

type ParseFn = fn(keys: &[csgo_proto::csvc_msg_game_event_list::KeyT], event: csgo_proto::CMsgSource1LegacyGameEvent) -> Result<GameEvent, ParseGameEventError>;

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
    inner: fn(keys: &[csgo_proto::csvc_msg_game_event_list::KeyT], event: csgo_proto::CMsgSource1LegacyGameEvent) -> Result<GameEvent, ParseGameEventError>,
}

impl GameEventParser {
    pub const fn new(func: ParseFn) -> Self {
        Self {
            inner: func,
        }
    }

    pub fn parse(&self, keys: &[csgo_proto::csvc_msg_game_event_list::KeyT], event: csgo_proto::CMsgSource1LegacyGameEvent) -> Result<GameEvent, ParseGameEventError> {
        if keys.len() != event.keys.len() {
            return Err(ParseGameEventError::MismatchedKeysFields);
        }

        (self.inner)(keys, event)
    }
}
