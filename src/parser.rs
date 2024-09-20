use crate::{packet::DemoEvent, DemoCommand, Frame, UserId};

mod fieldpath;
pub use fieldpath::{FieldPath, Paths};

mod decoder;
mod sendtables;
mod variant;

#[derive(Debug)]
pub enum FirstPassError {
    DecompressFrame,
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
pub struct FirstPassOutput {
    pub header: crate::csgo_proto::CDemoFileHeader,
    pub info: crate::csgo_proto::CDemoFileInfo,
    pub events: Vec<DemoEvent>,
    pub player_info: std::collections::HashMap<UserId, Player>,
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
    class_id: i32,
    name: String,
    serializer: sendtables::Serializer,
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
    let mut entities = std::collections::HashMap::new();
    let mut cls_to_class = std::collections::HashMap::<u32, Class>::new();
    let mut paths = Paths::new();
    let mut qf_mapper = decoder::QfMapper {
        idx: 0,
        map: std::collections::HashMap::new(),
    };
    let mut serializers = std::collections::HashMap::new();

    let mut baselines = std::collections::HashMap::new();

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
                parse_packet(
                    data,
                    &mut events,
                    &mut event_mapping,
                    &mut player_info,
                    &mut entities,
                    &mut cls_to_class,
                    &mut paths,
                    &mut qf_mapper,
                    &mut baselines,
                )?;
            }
            DemoCommand::FullPacket => {
                parse_fullpacket(
                    data,
                    &mut events,
                    &mut event_mapping,
                    &mut player_info,
                    &mut entities,
                    &mut cls_to_class,
                    &mut paths,
                    &mut qf_mapper,
                    &mut baselines,
                )?;
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
                serializers = sendtables::get_serializers(&serializer_msg, &mut qf_mapper)?;
            }
            DemoCommand::ClassInfo => {
                let raw: crate::csgo_proto::CDemoClassInfo = prost::Message::decode(data)?;

                cls_to_class.clear();

                for class_t in raw.classes {
                    let cls_id = class_t.class_id();
                    let network_name = class_t.network_name();

                    if let Some(ser) = serializers.remove(network_name) {
                        cls_to_class.insert(
                            cls_id as u32,
                            Class {
                                name: network_name.to_owned(),
                                class_id: cls_id,
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
    })
}

fn parse_fullpacket(
    data: &[u8],
    events: &mut Vec<DemoEvent>,
    event_mapper: &mut GameEventMapping,
    player_info: &mut std::collections::HashMap<UserId, Player>,
    entities: &mut std::collections::HashMap<i32, Entity>,
    cls_to_class: &mut std::collections::HashMap<u32, Class>,
    paths: &mut Paths,
    qf_mapper: &mut decoder::QfMapper,
    baselines: &mut std::collections::HashMap<u32, Vec<u8>>,
) -> Result<(), FirstPassError> {
    let raw: crate::csgo_proto::CDemoFullPacket = prost::Message::decode(data)?;

    // TODO
    // Handle string table stuff
    for item in raw.string_table.iter().flat_map(|st| st.tables.iter()) {
        // dbg!(&item.table_name);
    }

    match raw.packet {
        Some(packet) => {
            inner_parse_packet(
                &packet,
                events,
                event_mapper,
                player_info,
                entities,
                cls_to_class,
                paths,
                qf_mapper,
                baselines,
            )?;

            Ok(())
        }
        None => Ok(()),
    }
}

fn parse_packet(
    data: &[u8],
    events: &mut Vec<DemoEvent>,
    event_mapper: &mut GameEventMapping,
    player_info: &mut std::collections::HashMap<UserId, Player>,
    entities: &mut std::collections::HashMap<i32, Entity>,
    cls_to_class: &mut std::collections::HashMap<u32, Class>,
    paths: &mut Paths,
    qf_mapper: &mut decoder::QfMapper,
    baselines: &mut std::collections::HashMap<u32, Vec<u8>>,
) -> Result<(), FirstPassError> {
    let raw: crate::csgo_proto::CDemoPacket = prost::Message::decode(data)?;

    inner_parse_packet(
        &raw,
        events,
        event_mapper,
        player_info,
        entities,
        cls_to_class,
        paths,
        qf_mapper,
        baselines,
    )?;

    Ok(())
}

fn inner_parse_packet(
    raw: &crate::csgo_proto::CDemoPacket,
    events: &mut Vec<DemoEvent>,
    event_mapper: &mut GameEventMapping,
    player_info: &mut std::collections::HashMap<UserId, Player>,
    entities: &mut std::collections::HashMap<i32, Entity>,
    cls_to_class: &mut std::collections::HashMap<u32, Class>,
    paths: &mut Paths,
    qf_mapper: &mut decoder::QfMapper,
    baselines: &mut std::collections::HashMap<u32, Vec<u8>>,
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

                events.push(DemoEvent::ServerInfo(raw));
            }
            crate::netmessagetypes::NetmessageType::net_SignonState => {
                let raw: crate::csgo_proto::CnetMsgSignonState =
                    prost::Message::decode(msg_bytes.as_slice())?;
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
            crate::netmessagetypes::NetmessageType::svc_PacketEntities => {
                let raw: crate::csgo_proto::CsvcMsgPacketEntities =
                    prost::Message::decode(msg_bytes.as_slice())?;

                let mut bitreader = crate::bitreader::Bitreader::new(raw.entity_data());
                let mut entity_id: i32 = -1;
                for _ in 0..raw.updated_entries() {
                    entity_id += 1 + (bitreader.read_u_bit_var()? as i32);

                    match bitreader.read_nbits(2)? {
                        0b01 | 0b11 => {
                            entities.remove(&entity_id);
                        }
                        0b10 => {
                            let (id, entity) = create_entity(entity_id, &mut bitreader, baselines)?;
                            let cls = entity.cls;

                            entities.insert(entity_id, entity);
                            if let Some(baseline_bytes) = baselines.get(&cls) {
                                let mut br = crate::bitreader::Bitreader::new(&baseline_bytes);
                                update_entity(
                                    entity_id,
                                    &mut br,
                                    entities,
                                    cls_to_class,
                                    paths,
                                    qf_mapper,
                                )?;
                            }


                            update_entity(
                                entity_id,
                                &mut bitreader,
                                entities,
                                cls_to_class,
                                paths,
                                qf_mapper,
                            )?;
                        }
                        0b00 => {
                            if raw.has_pvs_vis_bits() > 0 {
                                if bitreader.read_nbits(2)? & 0x01 == 1 {
                                    continue;
                                }
                            }

                            update_entity(
                                entity_id,
                                &mut bitreader,
                                entities,
                                cls_to_class,
                                paths,
                                qf_mapper,
                            )?;
                        }
                        unknown => {
                            panic!("{:?}", unknown);
                        }
                    };
                }

                // dbg!("PacketEntities");
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
            crate::netmessagetypes::NetmessageType::CS_UM_PlayerStatsUpdate => {
                let raw: crate::csgo_proto::CcsUsrMsgPlayerStatsUpdate =
                    prost::Message::decode(msg_bytes.as_slice())?;
                // dbg!(&raw);
            }
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
            unknown => {
                dbg!(unknown);
            }
        };
    }

    Ok(())
}

fn create_entity(
    entity_id: i32,
    bitreader: &mut crate::bitreader::Bitreader,
    baselines: &mut std::collections::HashMap<u32, Vec<u8>>,
) -> Result<(i32, Entity), FirstPassError> {
    let cls_id: u32 = bitreader.read_nbits(8)?;
    let _serial = bitreader.read_nbits(17)?;
    let _unknown = bitreader.read_varint()?;

    Ok((entity_id, Entity { cls: cls_id }))
}

fn update_entity(
    entity_id: i32,
    bitreader: &mut crate::bitreader::Bitreader,
    entities: &mut std::collections::HashMap<i32, Entity>,
    cls_to_class: &mut std::collections::HashMap<u32, Class>,
    paths: &mut Paths,
    qf_mapper: &mut decoder::QfMapper,
) -> Result<(), FirstPassError> {
    let n_updates = fieldpath::parse_paths(bitreader, paths)?;
    let n_updated_values = decode_entity_update(
        entity_id,
        bitreader,
        n_updates,
        entities,
        cls_to_class,
        paths,
        qf_mapper,
    )?;
    if n_updated_values > 0 {
        gather_extra_info()?;
    }

    Ok(())
}

fn gather_extra_info() -> Result<(), FirstPassError> {
    // TODO

    Ok(())
}

fn decode_entity_update(
    entity_id: i32,
    bitreader: &mut crate::bitreader::Bitreader,
    n_updates: usize,
    entities: &mut std::collections::HashMap<i32, Entity>,
    cls_to_class: &mut std::collections::HashMap<u32, Class>,
    paths: &mut Paths,
    qf_mapper: &mut decoder::QfMapper,
) -> Result<usize, FirstPassError> {
    let entity = match entities.get_mut(&entity_id) {
        Some(e) => e,
        None => panic!("ID: {:?} - Entities: {:?}", entity_id, entities),
    };
    let class = match cls_to_class.get_mut(&entity.cls) {
        Some(c) => c,
        None => panic!(),
    };

    // dbg!(&class.name);
    for path in paths.paths().take(n_updates) {
        // dbg!(&path);

        let field = path.find(&class.serializer)?;
        let field_info = field.get_propinfo(path);
        let decoder = field.get_decoder()?;
        let result = decoder.decode(bitreader, qf_mapper)?;

        // dbg!(&field, &field_info, &decoder, &result);

        if let Some(fi) = field_info {
            // dbg!(&fi);
        }
    }

    Ok(n_updates)
}

static HUFFMAN_LOOKUP_TABLE: std::sync::LazyLock<Vec<(u8, u8)>> = std::sync::LazyLock::new(|| {
    let buf = include_bytes!("huf.b");
    let mut huf2 = Vec::with_capacity((1 << 17) - 1);
    for chunk in buf.chunks_exact(2) {
        huf2.push((chunk[0], chunk[1]));
    }
    huf2
});

fn do_op(
    symbol: u8,
    bitreader: &mut crate::bitreader::Bitreader,
    field_path: &mut FieldPath,
) -> Result<(), FirstPassError> {
    use fieldpath::ops::*;
    
    match symbol {
        0 => plus_one(bitreader, field_path),
        1 => plus_two(bitreader, field_path),
        2 => plus_three(bitreader, field_path),
        3 => plus_four(bitreader, field_path),
        4 => plus_n(bitreader, field_path),
        5 => push_one_left_delta_zero_right_zero(bitreader, field_path),
        6 => push_one_left_delta_zero_right_non_zero(bitreader, field_path),
        7 => push_one_left_delta_one_right_zero(bitreader, field_path),
        8 => push_one_left_delta_one_right_non_zero(bitreader, field_path),
        9 => push_one_left_delta_n_right_zero(bitreader, field_path),
        10 => push_one_left_delta_n_right_non_zero(bitreader, field_path),
        11 => push_one_left_delta_n_right_non_zero_pack6_bits(bitreader, field_path),
        12 => push_one_left_delta_n_right_non_zero_pack8_bits(bitreader, field_path),
        13 => push_two_left_delta_zero(bitreader, field_path),
        14 => push_two_pack5_left_delta_zero(bitreader, field_path),
        15 => push_three_left_delta_zero(bitreader, field_path),
        16 => push_three_pack5_left_delta_zero(bitreader, field_path),
        17 => push_two_left_delta_one(bitreader, field_path),
        18 => push_two_pack5_left_delta_one(bitreader, field_path),
        19 => push_three_left_delta_one(bitreader, field_path),
        20 => push_three_pack5_left_delta_one(bitreader, field_path),
        21 => push_two_left_delta_n(bitreader, field_path),
        22 => push_two_pack5_left_delta_n(bitreader, field_path),
        23 => push_three_left_delta_n(bitreader, field_path),
        24 => push_three_pack5_left_delta_n(bitreader, field_path),
        25 => push_n(bitreader, field_path),
        26 => push_n_and_non_topological(bitreader, field_path),
        27 => pop_one_plus_one(bitreader, field_path),
        28 => pop_one_plus_n(bitreader, field_path),
        29 => pop_all_but_one_plus_one(bitreader, field_path),
        30 => pop_all_but_one_plus_n(bitreader, field_path),
        31 => pop_all_but_one_plus_n_pack3_bits(bitreader, field_path),
        32 => pop_all_but_one_plus_n_pack6_bits(bitreader, field_path),
        33 => pop_n_plus_one(bitreader, field_path),
        34 => pop_n_plus_n(bitreader, field_path),
        35 => pop_n_and_non_topographical(bitreader, field_path),
        36 => non_topo_complex(bitreader, field_path),
        37 => non_topo_penultimate_plus_one(bitreader, field_path),
        38 => non_topo_complex_pack4_bits(bitreader, field_path),
        other => todo!("Other OP: {:?}", other),
    }
}
