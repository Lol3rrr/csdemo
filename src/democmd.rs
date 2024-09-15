#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum DemoCommand {
    Error,
    Stop,
    FileHeader,
    FileInfo,
    SyncTick,
    SendTables,
    ClassInfo,
    StringTables,
    Packet,
    SignonPacket,
    ConsoleCmd,
    CustomData,
    CustomDataCallbacks,
    UserCmd,
    FullPacket,
    SaveGame,
    SpawnGroups,
    AnimationData,
    AnimationHeader,
    Max,
    IsCompressed,
}

impl TryFrom<i32> for DemoCommand {
    type Error = i32;

    fn try_from(value: i32) -> Result<Self, i32> {
        match value {
            -1 => Ok(Self::Error),
            0 => Ok(Self::Stop),
            1 => Ok(Self::FileHeader),
            2 => Ok(Self::FileInfo),
            3 => Ok(Self::SyncTick),
            4 => Ok(Self::SendTables),
            5 => Ok(Self::ClassInfo),
            6 => Ok(Self::StringTables),
            7 => Ok(Self::Packet),
            8 => Ok(Self::SignonPacket),
            9 => Ok(Self::ConsoleCmd),
            10 => Ok(Self::CustomData),
            11 => Ok(Self::CustomDataCallbacks),
            12 => Ok(Self::UserCmd),
            13 => Ok(Self::FullPacket),
            14 => Ok(Self::SaveGame),
            15 => Ok(Self::SpawnGroups),
            16 => Ok(Self::AnimationData),
            17 => Ok(Self::AnimationHeader),
            18 => Ok(Self::Max),
            64 => Ok(Self::IsCompressed),
            unknown => Err(unknown),
        }
    }
}
