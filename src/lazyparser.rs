use crate::{Container, FrameIterator};

mod events;
pub use events::LazyEventIterator;

mod entities;
pub use entities::LazyEntityIterator;

pub struct LazyParser<'b> {
    container: Container<'b>,
}

impl<'b> LazyParser<'b> {
    pub fn new(container: Container<'b>) -> Self {
        Self { container }
    }

    pub fn file_header(&self) -> Option<crate::csgo_proto::CDemoFileHeader> {
        let mut buffer = Vec::new();

        for frame in FrameIterator::parse(self.container.inner) {
            if let crate::DemoCommand::FileHeader = frame.cmd {
                let data = frame.decompress_with_buf(&mut buffer).ok()?;
                let raw: crate::csgo_proto::CDemoFileHeader = prost::Message::decode(data).ok()?;
                return Some(raw);
            }
        }
        None
    }

    pub fn file_info(&self) -> Option<crate::csgo_proto::CDemoFileInfo> {
        let mut buffer = Vec::new();

        for frame in FrameIterator::parse(self.container.inner) {
            if let crate::DemoCommand::FileInfo = frame.cmd {
                let data = frame.decompress_with_buf(&mut buffer).ok()?;
                let raw: crate::csgo_proto::CDemoFileInfo = prost::Message::decode(data).ok()?;
                return Some(raw);
            }
        }
        None
    }

    pub fn player_info(&self) -> std::collections::HashMap<crate::UserId, crate::parser::Player> {
        let mut result = std::collections::HashMap::new();

        let mut buffer = Vec::new();

        for frame in FrameIterator::parse(self.container.inner) {
            let packet = match frame.cmd {
                crate::DemoCommand::Packet | crate::DemoCommand::SignonPacket => {
                    let data = match frame.decompress_with_buf(&mut buffer) {
                        Ok(d) => d,
                        Err(_) => continue,
                    };
                    let raw: crate::csgo_proto::CDemoPacket = match prost::Message::decode(data) {
                        Ok(d) => d,
                        Err(_) => continue,
                    };

                    raw
                }
                crate::DemoCommand::FullPacket => {
                    let data = match frame.decompress_with_buf(&mut buffer) {
                        Ok(d) => d,
                        Err(_) => continue,
                    };
                    let raw: crate::csgo_proto::CDemoFullPacket = match prost::Message::decode(data)
                    {
                        Ok(d) => d,
                        Err(_) => continue,
                    };

                    match raw.packet {
                        Some(p) => p,
                        None => continue,
                    }
                }
                _ => continue,
            };

            let mut bitreader = crate::bitreader::Bitreader::new(packet.data());

            while bitreader.bits_remaining().unwrap_or(0) > 8 {
                let msg_type = match bitreader.read_u_bit_var() {
                    Ok(t) => t,
                    Err(e) => break,
                };
                let size = match bitreader.read_varint() {
                    Ok(s) => s,
                    Err(e) => break,
                };
                let msg_bytes = match bitreader.read_n_bytes(size as usize) {
                    Ok(s) => s,
                    Err(e) => break,
                };

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
                    crate::netmessagetypes::NetmessageType::CS_UM_EndOfMatchAllPlayersData => {
                        let raw: crate::csgo_proto::CcsUsrMsgEndOfMatchAllPlayersData =
                            match prost::Message::decode(msg_bytes.as_slice()) {
                                Ok(d) => d,
                                Err(e) => continue,
                            };

                        for data in raw.allplayerdata {
                            result.insert(
                                crate::UserId(data.slot()),
                                crate::parser::Player {
                                    name: data.name.unwrap(),
                                    xuid: data.xuid.unwrap(),
                                    team: data.teamnumber.unwrap(),
                                    color: data.playercolor.unwrap(),
                                },
                            );
                        }
                    }
                    _unknown => {
                        // dbg!(unknown);
                    }
                };
            }
        }

        result
    }

    pub fn events(&self) -> LazyEventIterator<'b> {
        LazyEventIterator::new(self)
    }

    pub fn entities(&self) -> LazyEntityIterator<'b> {
        LazyEntityIterator::new(self)
    }
}
