use crate::{packet::DemoEvent, DemoCommand, Frame};

#[derive(Debug)]
pub enum FirstPassError {
    DecompressFrame,
    NoDataFrame,
    DecodeProtobuf(prost::DecodeError),
    MissingFileHeader,
    MissingFileInfo,
    Bitreader(crate::bitreader::BitReadError),
    ParseGameEventError(crate::game_event::ParseGameEventError)
}

impl From<prost::DecodeError> for FirstPassError {
    fn from(value: prost::DecodeError) -> Self {
        Self::DecodeProtobuf(value)
    }
}
impl From<crate::bitreader::BitReadError> for FirstPassError {
    fn from(value: crate::bitreader::BitReadError) -> Self {
        Self::Bitreader(value)
    }
}
impl From<crate::game_event::ParseGameEventError> for FirstPassError {
    fn from(value: crate::game_event::ParseGameEventError) -> Self {
        Self::ParseGameEventError(value)
    }
}

#[derive(Debug)]
pub struct Player {
    pub xuid: u64,
    pub name: String,
}

#[derive(Debug)]
pub struct FirstPassOutput {
    pub header: crate::csgo_proto::CDemoFileHeader,
    pub info: crate::csgo_proto::CDemoFileInfo,
    pub events: Vec<DemoEvent>,
    pub player_info: std::collections::HashMap<i32, Player>,
}

#[derive(Debug)]
struct GameEventMapping {
    mapping: std::collections::HashMap<i32, (String, Vec<crate::csgo_proto::csvc_msg_game_event_list::KeyT>)>,
}

pub fn parse<'b, FI>(frames: FI) -> Result<FirstPassOutput, FirstPassError>
where
    FI: IntoIterator<Item = Frame<'b>>,
{
    let mut header = None;
    let mut file_info = None;

    let mut events = Vec::new();
    let mut event_mapping = GameEventMapping {
        mapping: std::collections::HashMap::new(),
    };
    let mut player_info = std::collections::HashMap::new();

    for mut frame in frames.into_iter() {
        frame
            .decompress()
            .map_err(|e| FirstPassError::DecompressFrame)?;
        let data = frame.data().ok_or(FirstPassError::NoDataFrame)?;

        match frame.cmd {
            DemoCommand::FileHeader => {
                let raw: crate::csgo_proto::CDemoFileHeader = prost::Message::decode(data)?;
                header = Some(raw);
            }
            DemoCommand::FileInfo => {
                let raw: crate::csgo_proto::CDemoFileInfo = prost::Message::decode(data)?;
                file_info = Some(raw);
            }
            DemoCommand::SignonPacket | DemoCommand::Packet => {
                parse_packet(data, &mut events, &mut event_mapping, &mut player_info)?;
            }
            DemoCommand::FullPacket => {
                parse_fullpacket(data, &mut events, &mut event_mapping, &mut player_info)?;
            }
            _ => {}
        }
    }

    let header = header.ok_or(FirstPassError::MissingFileHeader)?;
    let info = file_info.ok_or(FirstPassError::MissingFileInfo)?;

    Ok(FirstPassOutput { header, info, events, player_info })
}

fn parse_fullpacket(
    data: &[u8],
    events: &mut Vec<DemoEvent>,
    event_mapper: &mut GameEventMapping,
    player_info: &mut std::collections::HashMap<i32, Player>,
) -> Result<(), FirstPassError> {
    let raw: crate::csgo_proto::CDemoFullPacket = prost::Message::decode(data)?;

    // TODO
    // Handle string table stuff

    match raw.packet {
        Some(packet) => {
            inner_parse_packet(&packet, events, event_mapper, player_info)?;

            Ok(())
        }
        None => Ok(()),
    }
}

fn parse_packet(
    data: &[u8],
    events: &mut Vec<DemoEvent>,
    event_mapper: &mut GameEventMapping,
    player_info: &mut std::collections::HashMap<i32, Player>,
) -> Result<(), FirstPassError> {
    let raw: crate::csgo_proto::CDemoPacket = prost::Message::decode(data)?;

    inner_parse_packet(&raw, events, event_mapper, player_info)?;

    Ok(())
}

fn inner_parse_packet(
    raw: &crate::csgo_proto::CDemoPacket,
    events: &mut Vec<DemoEvent>,
    event_mapper: &mut GameEventMapping,
    player_info: &mut std::collections::HashMap<i32, Player>,
) -> Result<(), FirstPassError> {
    let mut bitreader = crate::bitreader::Bitreader::new(raw.data());

    while bitreader.bits_remaining().unwrap_or(0) > 8 {
        let msg_type = bitreader.read_u_bit_var()?;
        let size = bitreader.read_varint()?;
        let msg_bytes = bitreader.read_n_bytes(size as usize)?;

        let net_msg_type = match crate::netmessagetypes::NetmessageType::try_from(msg_type as i32) {
            Ok(v) => v,
            Err(e) => {
                dbg!(e);
                continue;
            }
        };

        match net_msg_type {
            crate::netmessagetypes::NetmessageType::svc_ClearAllStringTables => {}
            crate::netmessagetypes::NetmessageType::svc_CreateStringTable => {}
            crate::netmessagetypes::NetmessageType::svc_UpdateStringTable => {}
            crate::netmessagetypes::NetmessageType::GE_Source1LegacyGameEventList => {
                let event_list: crate::csgo_proto::CsvcMsgGameEventList =
                    prost::Message::decode(msg_bytes.as_slice())?;

                event_mapper.mapping.clear();
                for event in event_list.descriptors {
                    event_mapper
                        .mapping
                        .insert(event.eventid(), (event.name().to_owned(), event.keys));
                }
            }
            crate::netmessagetypes::NetmessageType::svc_ServerInfo => {
                let raw: crate::csgo_proto::CsvcMsgServerInfo =
                    prost::Message::decode(msg_bytes.as_slice())?;

                events.push(DemoEvent::ServerInfo(raw));
            }
            crate::netmessagetypes::NetmessageType::net_SignonState => {
                let raw: crate::csgo_proto::CnetMsgSignonState = prost::Message::decode(msg_bytes.as_slice())?;
                dbg!(raw);
            }
            crate::netmessagetypes::NetmessageType::net_Tick => {
                let raw: crate::csgo_proto::CnetMsgTick =
                    prost::Message::decode(msg_bytes.as_slice())?;

                events.push(DemoEvent::Tick(raw));
            }
            crate::netmessagetypes::NetmessageType::net_SetConVar => {}
            crate::netmessagetypes::NetmessageType::svc_ClassInfo => {}
            crate::netmessagetypes::NetmessageType::svc_VoiceInit => {}
            crate::netmessagetypes::NetmessageType::svc_PacketEntities => {}
            crate::netmessagetypes::NetmessageType::svc_UserCmds => {}
            crate::netmessagetypes::NetmessageType::GE_SosStartSoundEvent => {}
            crate::netmessagetypes::NetmessageType::GE_SosStopSoundEvent => {}
            crate::netmessagetypes::NetmessageType::CS_GE_PlayerAnimationEvent => {}
            crate::netmessagetypes::NetmessageType::CS_GE_RadioIconEvent => {}
            crate::netmessagetypes::NetmessageType::CS_GE_FireBullets => {}
            crate::netmessagetypes::NetmessageType::GE_Source1LegacyGameEvent => {
                let raw: crate::csgo_proto::CMsgSource1LegacyGameEvent =
                    prost::Message::decode(msg_bytes.as_slice())?;

                match event_mapper.mapping.get(&raw.eventid()) {
                    Some((name, keys)) => {
                        match crate::game_event::EVENT_PARSERS.get(&name) {
                            Some(parser) => {
                                let parsed = parser.parse(keys.as_slice(), raw.clone())?;

                                events.push(DemoEvent::GameEvent(parsed));
                            }
                            None => {
                                println!("No parser for {:?}", name);
                            }
                        };
                    }
                    None => {
                        println!("Unknown Event - ID: {}", raw.eventid());
                    }
                };
            }
            crate::netmessagetypes::NetmessageType::UM_SayText2 => {}
            crate::netmessagetypes::NetmessageType::CS_UM_XpUpdate => {}
            crate::netmessagetypes::NetmessageType::CS_UM_ServerRankUpdate => {
                let raw: crate::csgo_proto::CcsUsrMsgServerRankUpdate =
                    prost::Message::decode(msg_bytes.as_slice())?;

                events.push(DemoEvent::RankUpdate(raw));
            }
            crate::netmessagetypes::NetmessageType::CS_UM_ServerRankRevealAll => {
                let raw: crate::csgo_proto::CcsUsrMsgServerRankRevealAll =
                    prost::Message::decode(msg_bytes.as_slice())?;

                events.push(DemoEvent::RankReveal(raw));
            }
            crate::netmessagetypes::NetmessageType::CS_UM_WeaponSound => {}
            crate::netmessagetypes::NetmessageType::CS_UM_RadioText => {}
            crate::netmessagetypes::NetmessageType::TE_WorldDecal => {}
            crate::netmessagetypes::NetmessageType::TE_EffectDispatch => {}
            crate::netmessagetypes::NetmessageType::CS_UM_PlayerStatsUpdate => {}
            crate::netmessagetypes::NetmessageType::CS_UM_EndOfMatchAllPlayersData => {
                let raw: crate::csgo_proto::CcsUsrMsgEndOfMatchAllPlayersData = prost::Message::decode(msg_bytes.as_slice())?;

                for data in raw.allplayerdata {
                    player_info.insert(data.slot(), Player {
                        name: data.name.unwrap(),
                        xuid: data.xuid.unwrap(),
                    });
                }
            }
            crate::netmessagetypes::NetmessageType::TE_PhysicsProp => {}
            crate::netmessagetypes::NetmessageType::UM_TextMsg => {}
            crate::netmessagetypes::NetmessageType::CS_UM_VoteFailed => {}
            crate::netmessagetypes::NetmessageType::net_SpawnGroup_Load => {}
            crate::netmessagetypes::NetmessageType::CS_UM_MatchEndConditions => {}
            crate::netmessagetypes::NetmessageType::TE_Explosion => {}
            unknown => {
                dbg!(unknown);
            }
        };
    }

    Ok(())
}
