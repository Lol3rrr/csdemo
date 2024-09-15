use std::io::Result;

fn main() -> Result<()> {
    prost_build::compile_protos(
        &[
            "Protobufs/csgo/demo.proto",
            "Protobufs/csgo/networkbasetypes.proto",
            "Protobufs/csgo/netmessages.proto",
            "Protobufs/csgo/gameevents.proto",
            "Protobufs/csgo/cstrike15_usermessages.proto",
        ],
        &["Protobufs/csgo"],
    )?;
    Ok(())
}
