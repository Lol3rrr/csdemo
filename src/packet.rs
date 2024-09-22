use crate::csgo_proto;

#[derive(Debug)]
pub enum DemoEvent {
    GameEvent(Box<crate::game_event::GameEvent>),
    ServerInfo(Box<csgo_proto::CsvcMsgServerInfo>),
    Tick(Box<csgo_proto::CnetMsgTick>),
    RankUpdate(Box<csgo_proto::CcsUsrMsgServerRankUpdate>),
    RankReveal(Box<csgo_proto::CcsUsrMsgServerRankRevealAll>),
}
