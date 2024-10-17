use crate::{
    parser::{
        decoder, entities, propcontroller, sendtables, update_entity, Class,
        FirstPassError, Paths,
    },
    DemoCommand, FrameIterator,
};

use std::collections::VecDeque;

pub struct LazyEntityIterator<'b> {
    buffer: Vec<u8>,
    frames: FrameIterator<'b>,

    current_tick: u32,
    pending_entities: VecDeque<(u32, entities::EntityState)>,

    paths: Paths,
    baselines: std::collections::HashMap<u32, Vec<u8>>,
    serializers: std::collections::HashMap<String, sendtables::Serializer>,
    qf_mapper: decoder::QfMapper,
    prop_controller: propcontroller::PropController,
    entity_ctx: entities::EntityContext,
}

impl<'b> LazyEntityIterator<'b> {
    pub(crate) fn new(parser: &super::LazyParser<'b>) -> Self {
        Self {
            buffer: Vec::new(),
            frames: FrameIterator::parse(parser.container.inner),

            current_tick: 0,
            pending_entities: VecDeque::with_capacity(64),

            paths: Paths::new(),
            baselines: std::collections::HashMap::new(),
            serializers: std::collections::HashMap::new(),
            qf_mapper: decoder::QfMapper {
                idx: 0,
                map: std::collections::HashMap::new(),
            },
            prop_controller: propcontroller::PropController::new(),
            entity_ctx: entities::EntityContext {
                entities: std::collections::HashMap::new(),
                cls_to_class: std::collections::HashMap::new(),
                filter: entities::EntityFilter::all(),
            },
        }
    }
}

impl<'b> LazyEntityIterator<'b> {
    fn inner_parse_packet(
        raw: &crate::csgo_proto::CDemoPacket,
        entity_ctx: &mut entities::EntityContext,
        paths: &mut Paths,
        qf_mapper: &mut decoder::QfMapper,
        baselines: &mut std::collections::HashMap<u32, Vec<u8>>,
        prop_controller: &propcontroller::PropController,
        entity_states: &mut VecDeque<(u32, entities::EntityState)>,
        current_tick: &mut u32,
    ) -> Result<(), FirstPassError> {
        let mut bitreader = crate::bitreader::Bitreader::new(raw.data());

        let mut msg_bytes = Vec::new();

        while bitreader.bits_remaining().unwrap_or(0) > 8 {
            let msg_type = bitreader.read_u_bit_var()?;
            let size = bitreader.read_varint()?;
            msg_bytes.clear();
            msg_bytes.resize(size as usize, 0);
            bitreader.read_n_bytes_mut(size as usize, &mut msg_bytes)?;

            assert_eq!(msg_bytes.len(), size as usize);

            let net_msg_type =
                match crate::netmessagetypes::NetmessageType::try_from(msg_type as i32) {
                    Ok(v) => v,
                    Err(e) => {
                        dbg!(e);
                        continue;
                    }
                };

            match net_msg_type {
                crate::netmessagetypes::NetmessageType::net_Tick => {
                    let raw: crate::csgo_proto::CnetMsgTick =
                        prost::Message::decode(msg_bytes.as_slice())?;

                    assert!(
                        *current_tick <= raw.tick(),
                        "Current Tick {} <= Tick Packet {}",
                        *current_tick,
                        raw.tick()
                    );
                    if raw.tick() > *current_tick {
                        *current_tick = raw.tick();
                    }
                }
                // TODO
                // How to handle these things?
                crate::netmessagetypes::NetmessageType::svc_ClearAllStringTables => {}
                crate::netmessagetypes::NetmessageType::svc_CreateStringTable => {}
                crate::netmessagetypes::NetmessageType::svc_UpdateStringTable => {}
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
                                    let cls =
                                        entity_ctx.create_entity(entity_id, &mut bitreader)?;

                                    if let Some(baseline_bytes) = baselines.get(&cls) {
                                        let mut br =
                                            crate::bitreader::Bitreader::new(baseline_bytes);

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
                                        entity_states.push_back((*current_tick, state));
                                    }
                                }
                                0b00 => {
                                    if raw.has_pvs_vis_bits() > 0
                                        && bitreader.read_nbits(2)? & 0x01 == 1
                                    {
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
                                        entity_states.push_back((*current_tick, state));
                                    }
                                }
                                unknown => {
                                    panic!("{:?}", unknown);
                                }
                            };
                        }
                    }
                }
                _ => {
                    // dbg!(unknown);
                }
            };
        }

        Ok(())
    }
}

impl<'b> Iterator for LazyEntityIterator<'b> {
    type Item = Result<(u32, crate::parser::entities::EntityState), ()>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(tmp) = self.pending_entities.pop_front() {
            return Some(Ok(tmp));
        }

        while let Some(frame) = self.frames.next() {
            match frame.cmd {
                DemoCommand::SignonPacket | DemoCommand::Packet => {
                    let data = match frame.decompress_with_buf(&mut self.buffer) {
                        Ok(d) => d,
                        Err(e) => return Some(Err(())),
                    };
                    let raw: crate::csgo_proto::CDemoPacket = match prost::Message::decode(data) {
                        Ok(d) => d,
                        Err(e) => return Some(Err(())),
                    };

                    if let Err(e) = Self::inner_parse_packet(
                        &raw,
                        &mut self.entity_ctx,
                        &mut self.paths,
                        &mut self.qf_mapper,
                        &mut self.baselines,
                        &mut self.prop_controller,
                        &mut self.pending_entities,
                        &mut self.current_tick,
                    ) {
                        return Some(Err(()));
                    }
                }
                DemoCommand::FullPacket => {
                    let data = match frame.decompress_with_buf(&mut self.buffer) {
                        Ok(d) => d,
                        Err(e) => return Some(Err(())),
                    };
                    let raw: crate::csgo_proto::CDemoFullPacket = match prost::Message::decode(data)
                    {
                        Ok(d) => d,
                        Err(e) => return Some(Err(())),
                    };

                    if let Some(packet) = raw.packet {
                        if let Err(e) = Self::inner_parse_packet(
                            &packet,
                            &mut self.entity_ctx,
                            &mut self.paths,
                            &mut self.qf_mapper,
                            &mut self.baselines,
                            &mut self.prop_controller,
                            &mut self.pending_entities,
                            &mut self.current_tick,
                        ) {
                            return Some(Err(()));
                        }
                    }
                }

                // Handling all the "meta" related stuff
                DemoCommand::StringTables => {
                    let data = match frame.decompress_with_buf(&mut self.buffer) {
                        Ok(d) => d,
                        Err(e) => return Some(Err(())),
                    };
                    let raw: crate::csgo_proto::CDemoStringTables =
                        match prost::Message::decode(data) {
                            Ok(d) => d,
                            Err(e) => return Some(Err(())),
                        };

                    for table in raw.tables.into_iter() {
                        if table.table_name() == "instancebaseline" {
                            for item in table.items.into_iter() {
                                let k = item.str().parse::<u32>().unwrap_or(u32::MAX);
                                self.baselines.insert(k, item.data.unwrap_or(Vec::new()));
                            }
                        }
                    }
                }
                DemoCommand::SendTables => {
                    let data = match frame.decompress_with_buf(&mut self.buffer) {
                        Ok(d) => d,
                        Err(e) => return Some(Err(())),
                    };
                    let tables: crate::csgo_proto::CDemoSendTables =
                        match prost::Message::decode(data) {
                            Ok(d) => d,
                            Err(e) => return Some(Err(())),
                        };

                    let mut bitreader = crate::bitreader::Bitreader::new(tables.data());

                    let n_bytes = match bitreader.read_varint() {
                        Ok(b) => b,
                        Err(e) => return Some(Err(())),
                    };
                    let bytes = match bitreader.read_n_bytes(n_bytes as usize) {
                        Ok(b) => b,
                        Err(e) => return Some(Err(())),
                    };

                    let serializer_msg: crate::csgo_proto::CsvcMsgFlattenedSerializer =
                        match prost::Message::decode(bytes.as_slice()) {
                            Ok(d) => d,
                            Err(e) => return Some(Err(())),
                        };

                    // std::fs::write("send_table.b", bytes.as_slice());

                    assert!(self.serializers.is_empty());
                    self.serializers = match sendtables::get_serializers(
                        &serializer_msg,
                        &mut self.qf_mapper,
                        &mut self.prop_controller,
                    ) {
                        Ok(s) => s,
                        Err(e) => return Some(Err(())),
                    };
                }
                DemoCommand::ClassInfo => {
                    let data = match frame.decompress_with_buf(&mut self.buffer) {
                        Ok(d) => d,
                        Err(e) => return Some(Err(())),
                    };
                    let raw: crate::csgo_proto::CDemoClassInfo = match prost::Message::decode(data)
                    {
                        Ok(d) => d,
                        Err(e) => return Some(Err(())),
                    };

                    self.entity_ctx.cls_to_class.clear();

                    for class_t in raw.classes {
                        let cls_id = class_t.class_id();
                        let network_name = class_t.network_name();

                        if let Some(ser) = self.serializers.remove(network_name) {
                            self.entity_ctx.cls_to_class.insert(
                                cls_id as u32,
                                Class {
                                    name: network_name.into(),
                                    serializer: ser,
                                },
                            );
                        }
                    }
                }
                _ => continue,
            };

            if let Some(tmp) = self.pending_entities.pop_front() {
                return Some(Ok(tmp));
            }
        }

        None
    }
}
