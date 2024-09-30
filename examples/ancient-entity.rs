const DATA: &[u8] = include_bytes!("../testfiles/de_ancient.dem");

fn main() {
    let container = csdemo::Container::parse(DATA).unwrap();

    let output = csdemo::parser::parse(
        csdemo::FrameIterator::parse(container.inner),
        csdemo::parser::EntityFilter::all(),
    )
    .unwrap();

    println!("Header: {:?}", output.header);
    println!("Players: {:?}", output.player_info);
    println!("Events: {:?}", output.events.len());
    println!("Entity-Ticks: {:?}", output.entity_states.ticks.len());

    /*
    for tick in output.entity_states.ticks.iter() {
        for state in tick.states.iter() {
            if state.class.as_str() != "CCSPlayerPawn" {
                continue;
            }

            for prop in state.props.iter() {
                println!("{:?} = {:?}", prop.prop_info.prop_name, prop.value);
            }
        }
    }
    */
}
