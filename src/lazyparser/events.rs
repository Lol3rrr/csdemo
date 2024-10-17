use crate::{parser::GameEventMapping, DemoEvent, FrameIterator};

use std::collections::VecDeque;

pub struct LazyEventIterator<'b> {
    pub(super) buffer: Vec<u8>,
    pub(super) frames: FrameIterator<'b>,

    pub(super) pending_events: VecDeque<crate::DemoEvent>,
    pub(super) event_mapper: GameEventMapping,
}

impl<'b> LazyEventIterator<'b> {
    pub(crate) fn new(parser: &super::LazyParser<'b>) -> Self {
        Self {
            buffer: Vec::new(),
            frames: FrameIterator::parse(parser.container.inner),

            pending_events: VecDeque::with_capacity(64),
            event_mapper: crate::parser::GameEventMapping {
                mapping: std::collections::HashMap::new(),
            },
        }
    }
}

impl<'b> LazyEventIterator<'b> {
    fn inner_parse_packet(
        raw: &crate::csgo_proto::CDemoPacket,
        events: &mut VecDeque<DemoEvent>,
        event_mapper: &mut GameEventMapping,
    ) -> Result<(), ()> {
        let mut bitreader = crate::bitreader::Bitreader::new(raw.data());

        while bitreader.bits_remaining().unwrap_or(0) > 8 {
            let msg_type = bitreader.read_u_bit_var().map_err(|e| ())?;
            let size = bitreader.read_varint().map_err(|e| ())?;
            let msg_bytes = bitreader.read_n_bytes(size as usize).map_err(|e| ())?;

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
                crate::netmessagetypes::NetmessageType::GE_Source1LegacyGameEventList => {
                    let event_list: crate::csgo_proto::CsvcMsgGameEventList =
                        prost::Message::decode(msg_bytes.as_slice()).map_err(|e| ())?;

                    event_mapper.mapping.clear();
                    for event in event_list.descriptors {
                        event_mapper
                            .mapping
                            .insert(event.eventid(), (event.name().to_owned(), event.keys));
                    }
                }
                crate::netmessagetypes::NetmessageType::svc_ServerInfo => {
                    let raw: crate::csgo_proto::CsvcMsgServerInfo =
                        prost::Message::decode(msg_bytes.as_slice()).map_err(|e| ())?;

                    events.push_back(DemoEvent::ServerInfo(Box::new(raw)));
                }
                crate::netmessagetypes::NetmessageType::net_Tick => {
                    let raw: crate::csgo_proto::CnetMsgTick =
                        prost::Message::decode(msg_bytes.as_slice()).map_err(|e| ())?;

                    events.push_back(DemoEvent::Tick(Box::new(raw)));
                }
                crate::netmessagetypes::NetmessageType::GE_Source1LegacyGameEvent => {
                    let raw: crate::csgo_proto::CMsgSource1LegacyGameEvent =
                        prost::Message::decode(msg_bytes.as_slice()).map_err(|e| ())?;

                    match event_mapper.mapping.get(&raw.eventid()) {
                        Some((name, keys)) => {
                            match crate::game_event::EVENT_PARSERS.get(name) {
                                Some(parser) => {
                                    let parsed = parser
                                        .parse(keys.as_slice(), raw.clone())
                                        .map_err(|e| ())?;

                                    events.push_back(DemoEvent::GameEvent(Box::new(parsed)));
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
                crate::netmessagetypes::NetmessageType::CS_UM_ServerRankUpdate => {
                    let raw: crate::csgo_proto::CcsUsrMsgServerRankUpdate =
                        prost::Message::decode(msg_bytes.as_slice()).map_err(|e| ())?;

                    events.push_back(DemoEvent::RankUpdate(Box::new(raw)));
                }
                crate::netmessagetypes::NetmessageType::CS_UM_ServerRankRevealAll => {
                    let raw: crate::csgo_proto::CcsUsrMsgServerRankRevealAll =
                        prost::Message::decode(msg_bytes.as_slice()).map_err(|e| ())?;

                    events.push_back(DemoEvent::RankReveal(Box::new(raw)));
                }
                crate::netmessagetypes::NetmessageType::net_SignonState
                | crate::netmessagetypes::NetmessageType::svc_ClearAllStringTables
                | crate::netmessagetypes::NetmessageType::svc_CreateStringTable
                | crate::netmessagetypes::NetmessageType::svc_UpdateStringTable
                | crate::netmessagetypes::NetmessageType::net_SetConVar
                | crate::netmessagetypes::NetmessageType::svc_ClassInfo
                | crate::netmessagetypes::NetmessageType::svc_VoiceInit
                | crate::netmessagetypes::NetmessageType::svc_PacketEntities
                | crate::netmessagetypes::NetmessageType::svc_UserCmds
                | crate::netmessagetypes::NetmessageType::GE_SosStartSoundEvent
                | crate::netmessagetypes::NetmessageType::GE_SosStopSoundEvent
                | crate::netmessagetypes::NetmessageType::CS_GE_PlayerAnimationEvent
                | crate::netmessagetypes::NetmessageType::CS_GE_RadioIconEvent
                | crate::netmessagetypes::NetmessageType::CS_GE_FireBullets
                | crate::netmessagetypes::NetmessageType::UM_SayText2
                | crate::netmessagetypes::NetmessageType::CS_UM_XpUpdate
                | crate::netmessagetypes::NetmessageType::CS_UM_WeaponSound
                | crate::netmessagetypes::NetmessageType::CS_UM_RadioText
                | crate::netmessagetypes::NetmessageType::TE_WorldDecal
                | crate::netmessagetypes::NetmessageType::TE_EffectDispatch
                | crate::netmessagetypes::NetmessageType::CS_UM_EndOfMatchAllPlayersData
                | crate::netmessagetypes::NetmessageType::TE_PhysicsProp
                | crate::netmessagetypes::NetmessageType::UM_TextMsg
                | crate::netmessagetypes::NetmessageType::CS_UM_VoteFailed
                | crate::netmessagetypes::NetmessageType::net_SpawnGroup_Load
                | crate::netmessagetypes::NetmessageType::CS_UM_MatchEndConditions
                | crate::netmessagetypes::NetmessageType::TE_Explosion => {}
                _unknown => {
                    // dbg!(unknown);
                }
            };
        }

        Ok(())
    }
}

impl<'b> Iterator for LazyEventIterator<'b> {
    type Item = Result<crate::DemoEvent, ()>;

    fn next(&mut self) -> Option<Self::Item> {
        use crate::DemoCommand;

        if let Some(event) = self.pending_events.pop_front() {
            return Some(Ok(event));
        }

        while let Some(frame) = self.frames.next() {
            match frame.cmd {
                DemoCommand::SignonPacket | DemoCommand::Packet => {
                    let data = match frame.decompress_with_buf(&mut self.buffer) {
                        Ok(d) => d,
                        Err(e) => return Some(Err(())),
                    };

                    let raw: crate::csgo_proto::CDemoPacket = match prost::Message::decode(data) {
                        Ok(p) => p,
                        Err(e) => return Some(Err(())),
                    };

                    if let Err(e) = Self::inner_parse_packet(
                        &raw,
                        &mut self.pending_events,
                        &mut self.event_mapper,
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
                        Ok(p) => p,
                        Err(e) => return Some(Err(())),
                    };

                    // TODO

                    if let Some(packet) = raw.packet {
                        if let Err(e) = Self::inner_parse_packet(
                            &packet,
                            &mut self.pending_events,
                            &mut self.event_mapper,
                        ) {
                            return Some(Err(()));
                        }
                    }
                }
                _ => {}
            };

            if let Some(event) = self.pending_events.pop_front() {
                return Some(Ok(event));
            }
        }

        None
    }
}
