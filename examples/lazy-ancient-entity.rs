const DATA: &[u8] = include_bytes!("../testfiles/de_ancient.dem");

fn main() {
    let container = csdemo::Container::parse(DATA).unwrap();

    let demo = csdemo::lazyparser::LazyParser::new(container);

    for entity in demo.entities() {
        core::hint::black_box(entity);
    }

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
