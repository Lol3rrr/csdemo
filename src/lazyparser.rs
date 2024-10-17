use crate::{Container, FrameIterator};

use std::collections::VecDeque;

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

    pub fn events(&self) -> LazyEventIterator<'b> {
        LazyEventIterator::new(self)
    }

    pub fn entities(&self) -> LazyEntityIterator<'b> {
        LazyEntityIterator::new(self)
    }
}
