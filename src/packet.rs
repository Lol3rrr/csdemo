use crate::csgo_proto;

#[derive(Debug)]
pub enum DemoEvent {
    GameEvent(crate::game_event::GameEvent),
    ServerInfo(csgo_proto::CsvcMsgServerInfo),
    Tick(csgo_proto::CnetMsgTick),
    RankUpdate(csgo_proto::CcsUsrMsgServerRankUpdate),
    RankReveal(csgo_proto::CcsUsrMsgServerRankRevealAll),
}
