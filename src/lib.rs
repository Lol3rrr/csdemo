mod container;
pub use container::{Container, ParseContainerError};

mod frame;
pub use frame::{Frame, FrameIterator};

mod democmd;
pub use democmd::DemoCommand;

mod netmessagetypes;

mod bitreader;
mod varint;

mod packet;
pub use packet::DemoEvent;
pub mod game_event;

pub mod parser;

pub mod csgo_proto {
    include!(concat!(env!("OUT_DIR"), "/_.rs"));
}
