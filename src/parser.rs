use crate::{packet::DemoEvent, DemoCommand, Frame, UserId, FrameDecompressError};

mod fieldpath;
pub use fieldpath::{FieldPath, Paths};

mod decoder;
mod entities;
mod propcontroller;
mod sendtables;
mod variant;

pub use entities::EntityFilter;

#[derive(Debug)]
pub enum FirstPassError {
    DecompressFrame(FrameDecompressError),
    NoDataFrame,
    DecodeProtobuf(prost::DecodeError),
    MissingFileHeader,
    MissingFileInfo,
    Bitreader(crate::bitreader::BitReadError),
    ParseGameEventError(crate::game_event::ParseGameEventError),
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
    pub team: i32,
    pub color: i32,
}

#[derive(Debug)]
pub struct Entity {
    pub cls: u32,
}

#[derive(Debug)]
pub struct EntityTickList {
    pub ticks: Vec<EntityTickStates>,
}

impl EntityTickList {
    pub fn new() -> Self {
        Self {
            ticks: vec![EntityTickStates {
                tick: 0,
                states: Vec::new()
            }],
        }
    }

    fn new_tick(&mut self, n_tick: u32) {
        self.ticks.push(EntityTickStates {
            tick: n_tick,
            states: Vec::new(),
        });
    }

    fn add_state(&mut self, state: entities::EntityState) {
        self.ticks.last_mut().expect("We start with at least 1 tick entry and never remove any, so we can always get the last element").states.push(state);
    }
}

#[derive(Debug)]
pub struct EntityTickStates {
    pub tick: u32,
    pub states: Vec<entities::EntityState>,
}

#[derive(Debug)]
pub struct FirstPassOutput {
    pub header: crate::csgo_proto::CDemoFileHeader,
    pub info: crate::csgo_proto::CDemoFileInfo,
    pub events: Vec<DemoEvent>,
    pub player_info: std::collections::HashMap<UserId, Player>,
    pub entity_states: EntityTickList,
}

#[derive(Debug)]
struct GameEventMapping {
    mapping: std::collections::HashMap<
        i32,
        (
            String,
            Vec<crate::csgo_proto::csvc_msg_game_event_list::KeyT>,
        ),
    >,
}

#[derive(Debug)]
pub struct Class {
    name: String,
    serializer: sendtables::Serializer,
}

pub fn parse<'b, FI>(frames: FI, filter: EntityFilter) -> Result<FirstPassOutput, FirstPassError>
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
    let mut entity_ctx = entities::EntityContext {
        entities: std::collections::HashMap::new(),
        cls_to_class: std::collections::HashMap::new(),
        filter,
    };
    let mut paths = Paths::new();
    let mut qf_mapper = decoder::QfMapper {
        idx: 0,
        map: std::collections::HashMap::new(),
    };
    let mut prop_controller = propcontroller::PropController::new();
    let mut serializers = std::collections::HashMap::new();

    let mut baselines = std::collections::HashMap::new();

    let mut entity_states = EntityTickList::new();

    let mut current_tick = 0;

    let mut buffer = Vec::new();
    for frame in frames.into_iter() {
        let data = frame
            .decompress_with_buf(&mut buffer)
            .map_err(FirstPassError::DecompressFrame)?;

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
                let raw: crate::csgo_proto::CDemoPacket = prost::Message::decode(data)?;

                inner_parse_packet(
                    &raw,
                    &mut events,
                    &mut event_mapping,
                    &mut player_info,
                    &mut entity_ctx,
                    &mut paths,
                    &mut qf_mapper,
                    &mut baselines,
                    &prop_controller,
                    &mut entity_states,
                    &mut current_tick,
                )?;
            }
            DemoCommand::FullPacket => {
                let packet = parse_fullpacket(data)?;

                if let Some(packet) = packet {
                    inner_parse_packet(
                        &packet,
                        &mut events,
                        &mut event_mapping,
                        &mut player_info,
                        &mut entity_ctx,
                        &mut paths,
                        &mut qf_mapper,
                        &mut baselines,
                        &prop_controller,
                        &mut entity_states,
                        &mut current_tick,
                    )?;
                }
            }
            // TODO
            DemoCommand::AnimationData => {}
            DemoCommand::AnimationHeader => {}
            DemoCommand::StringTables => {
                let raw: crate::csgo_proto::CDemoStringTables = prost::Message::decode(data)?;

                for table in raw.tables.iter() {
                    if table.table_name() == "instancebaseline" {
                        for item in table.items.iter() {
                            let k = item.str().parse::<u32>().unwrap_or(u32::MAX);
                            baselines.insert(k, item.data().to_vec());
                        }
                    }
                }
            }
            DemoCommand::SendTables => {
                let tables: crate::csgo_proto::CDemoSendTables = prost::Message::decode(data)?;

                let mut bitreader = crate::bitreader::Bitreader::new(tables.data());

                let n_bytes = bitreader.read_varint()?;
                let bytes = bitreader.read_n_bytes(n_bytes as usize)?;

                let serializer_msg: crate::csgo_proto::CsvcMsgFlattenedSerializer =
                    prost::Message::decode(bytes.as_slice())?;

                // std::fs::write("send_table.b", bytes.as_slice());

                assert!(serializers.is_empty());
                serializers = sendtables::get_serializers(
                    &serializer_msg,
                    &mut qf_mapper,
                    &mut prop_controller,
                )?;
            }
            DemoCommand::ClassInfo => {
                let raw: crate::csgo_proto::CDemoClassInfo = prost::Message::decode(data)?;

                entity_ctx.cls_to_class.clear();

                for class_t in raw.classes {
                    let cls_id = class_t.class_id();
                    let network_name = class_t.network_name();

                    if let Some(ser) = serializers.remove(network_name) {
                        entity_ctx.cls_to_class.insert(
                            cls_id as u32,
                            Class {
                                name: network_name.to_owned(),
                                serializer: ser,
                            },
                        );
                    }
                }
            }
            other => {
                dbg!(other);
            }
        }
    }

    let header = header.ok_or(FirstPassError::MissingFileHeader)?;
    let info = file_info.ok_or(FirstPassError::MissingFileInfo)?;

    Ok(FirstPassOutput {
        header,
        info,
        events,
        player_info,
        entity_states,
    })
}

fn parse_fullpacket(data: &[u8]) -> Result<Option<crate::csgo_proto::CDemoPacket>, FirstPassError> {
    let raw: crate::csgo_proto::CDemoFullPacket = prost::Message::decode(data)?;

    // TODO
    // Handle string table stuff
    for _item in raw.string_table.iter().flat_map(|st| st.tables.iter()) {
        // dbg!(&item.table_name);
    }

    Ok(raw.packet)
}

fn inner_parse_packet(
    raw: &crate::csgo_proto::CDemoPacket,
    events: &mut Vec<DemoEvent>,
    event_mapper: &mut GameEventMapping,
    player_info: &mut std::collections::HashMap<UserId, Player>,
    entity_ctx: &mut entities::EntityContext,
    paths: &mut Paths,
    qf_mapper: &mut decoder::QfMapper,
    baselines: &mut std::collections::HashMap<u32, Vec<u8>>,
    prop_controller: &propcontroller::PropController,
    entity_states: &mut EntityTickList,
    current_tick: &mut u32,
) -> Result<(), FirstPassError> {
    let mut bitreader = crate::bitreader::Bitreader::new(raw.data());

    while bitreader.bits_remaining().unwrap_or(0) > 8 {
        let msg_type = bitreader.read_u_bit_var()?;
        let size = bitreader.read_varint()?;
        let msg_bytes = bitreader.read_n_bytes(size as usize)?;

        assert_eq!(msg_bytes.len(), size as usize);

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

                events.push(DemoEvent::ServerInfo(Box::new(raw)));
            }
            crate::netmessagetypes::NetmessageType::net_SignonState => {
                let raw: crate::csgo_proto::CnetMsgSignonState =
                    prost::Message::decode(msg_bytes.as_slice())?;
                dbg!(raw);
            }
            crate::netmessagetypes::NetmessageType::net_Tick => {
                let raw: crate::csgo_proto::CnetMsgTick =
                    prost::Message::decode(msg_bytes.as_slice())?;

                assert!(*current_tick <= raw.tick(), "Current Tick {} <= Tick Packet {}", *current_tick, raw.tick());
                if raw.tick() > *current_tick {
                    *current_tick = raw.tick();
                    entity_states.new_tick(*current_tick);
                }

                events.push(DemoEvent::Tick(Box::new(raw)));
            }
            crate::netmessagetypes::NetmessageType::net_SetConVar => {}
            crate::netmessagetypes::NetmessageType::svc_ClassInfo => {}
            crate::netmessagetypes::NetmessageType::svc_VoiceInit => {}
            crate::netmessagetypes::NetmessageType::svc_PacketEntities => {
                let raw: crate::csgo_proto::CsvcMsgPacketEntities =
                    prost::Message::decode(msg_bytes.as_slice())?;

                if entity_ctx.filter.enabled {
                    let mut bitreader = crate::bitreader::Bitreader::new(raw.entity_data());
                    let mut entity_id: i32 = -1;
                    for _ in 0..raw.updated_entries() {
                        entity_id += 1 + (bitreader.read_u_bit_var()? as i32);

                        match bitreader.read_nbits(2)? {
                            0b01 | 0b11 => {
                                entity_ctx.entities.remove(&entity_id);
                            }
                            0b10 => {
                                let cls = entity_ctx.create_entity(entity_id, &mut bitreader)?;

                                if let Some(baseline_bytes) = baselines.get(&cls) {
                                    let mut br = crate::bitreader::Bitreader::new(baseline_bytes);
                                    
                                    // TODO
                                    // How should we handle is this?
                                    let _state = update_entity(
                                        entity_id,
                                        &mut br,
                                        entity_ctx,
                                        paths,
                                        qf_mapper,
                                        prop_controller,
                                    )?;
                                }

                                let state = update_entity(
                                    entity_id,
                                    &mut bitreader,
                                    entity_ctx,
                                    paths,
                                    qf_mapper,
                                    prop_controller,
                                )?;
                                if let Some(state) = state {
                                    entity_states.add_state(state);
                                }
                            }
                            0b00 => {
                                if raw.has_pvs_vis_bits() > 0 && bitreader.read_nbits(2)? & 0x01 == 1 {
                                    continue;
                                }

                                let state = update_entity(
                                    entity_id,
                                    &mut bitreader,
                                    entity_ctx,
                                    paths,
                                    qf_mapper,
                                    prop_controller,
                                )?;
                                if let Some(state) = state {
                                    entity_states.add_state(state);
                                }
                            }
                            unknown => {
                                panic!("{:?}", unknown);
                            }
                        };
                    }
                }
            }
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
                        match crate::game_event::EVENT_PARSERS.get(name) {
                            Some(parser) => {
                                let parsed = parser.parse(keys.as_slice(), raw.clone())?;

                                events.push(DemoEvent::GameEvent(Box::new(parsed)));
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

                events.push(DemoEvent::RankUpdate(Box::new(raw)));
            }
            crate::netmessagetypes::NetmessageType::CS_UM_ServerRankRevealAll => {
                let raw: crate::csgo_proto::CcsUsrMsgServerRankRevealAll =
                    prost::Message::decode(msg_bytes.as_slice())?;

                events.push(DemoEvent::RankReveal(Box::new(raw)));
            }
            crate::netmessagetypes::NetmessageType::CS_UM_WeaponSound => {}
            crate::netmessagetypes::NetmessageType::CS_UM_RadioText => {}
            crate::netmessagetypes::NetmessageType::TE_WorldDecal => {}
            crate::netmessagetypes::NetmessageType::TE_EffectDispatch => {}
            crate::netmessagetypes::NetmessageType::CS_UM_EndOfMatchAllPlayersData => {
                let raw: crate::csgo_proto::CcsUsrMsgEndOfMatchAllPlayersData =
                    prost::Message::decode(msg_bytes.as_slice())?;

                for data in raw.allplayerdata {
                    player_info.insert(
                        UserId(data.slot()),
                        Player {
                            name: data.name.unwrap(),
                            xuid: data.xuid.unwrap(),
                            team: data.teamnumber.unwrap(),
                            color: data.playercolor.unwrap(),
                        },
                    );
                }
            }
            crate::netmessagetypes::NetmessageType::TE_PhysicsProp => {}
            crate::netmessagetypes::NetmessageType::UM_TextMsg => {}
            crate::netmessagetypes::NetmessageType::CS_UM_VoteFailed => {}
            crate::netmessagetypes::NetmessageType::net_SpawnGroup_Load => {}
            crate::netmessagetypes::NetmessageType::CS_UM_MatchEndConditions => {}
            crate::netmessagetypes::NetmessageType::TE_Explosion => {}
            _unknown => {
                // dbg!(unknown);
            }
        };
    }

    Ok(())
}

fn update_entity(
    entity_id: i32,
    bitreader: &mut crate::bitreader::Bitreader,
    entity_ctx: &mut entities::EntityContext,
    paths: &mut Paths,
    qf_mapper: &mut decoder::QfMapper,
    prop_controller: &propcontroller::PropController,
) -> Result<Option<entities::EntityState>, FirstPassError> {
    let n_updates = fieldpath::parse_paths(bitreader, paths)?;
    let (n_updated_values, entity_state) = match entity_ctx.decode_entity_update(
        entity_id,
        bitreader,
        n_updates,
        paths,
        qf_mapper,
        prop_controller,
    )? {
        Some(s) => s,
        None => return Ok(None),
    };
    if n_updated_values > 0 {
        // TODO
        // Gather extra information
        // gather_extra_info(entity_id, prop_controller)?;
    }

    Ok(Some(entity_state))
}

static HUFFMAN_LOOKUP_TABLE: std::sync::LazyLock<Vec<(u8, u8)>> = std::sync::LazyLock::new(|| {
    let buf = include_bytes!("huf.b");
    let mut huf2 = Vec::with_capacity((1 << 17) - 1);
    for chunk in buf.chunks_exact(2) {
        huf2.push((chunk[0], chunk[1]));
    }
    huf2
});
