const DATA: &[u8] = include_bytes!("../testfiles/de_ancient.dem");

fn main() {
    let container = csdemo::Container::parse(DATA).unwrap();

    let output = csdemo::parser::parse(csdemo::FrameIterator::parse(container.inner), csdemo::parser::EntityFilter::disabled()).unwrap();

    println!("Header: {:?}", output.header);
    println!("Players: {:?}", output.player_info);
    println!("Events: {:?}", output.events.len());
    println!("Entities: {:?}", output.entity_states.len());
}
