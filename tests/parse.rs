use csdemo::{game_event::GameEvent, DemoEvent};

#[test]
fn mirage_1() {
    let content = std::fs::read("testfiles/mirage.dem").unwrap();

    let container = csdemo::Container::parse(&content).unwrap();

    let frame_iter = csdemo::FrameIterator::parse(container.inner);
    assert_eq!(123333, frame_iter.count());

    let output = csdemo::parser::parse(csdemo::FrameIterator::parse(container.inner)).unwrap();

    assert_eq!("de_mirage", output.header.map_name());

    for event in output.events.iter() {
        match event {
            DemoEvent::GameEvent(gevent) => match gevent {
                GameEvent::PlayerDeath(death) => {
                    assert!(
                        death.remaining.is_empty(),
                        "Remaining for PlayerDeath: {:?}",
                        death.remaining
                    );

                    let died_user = output
                        .player_info
                        .get(death.userid.as_ref().unwrap())
                        .unwrap();
                    // dbg!(died_user);
                }
                _ => {}
            },
            _ => {}
        };
    }

    todo!()
}

#[test]
fn ancient_1() {
    let content = std::fs::read("testfiles/de_ancient.dem").unwrap();

    let container = csdemo::Container::parse(&content).unwrap();

    let frame_iter = csdemo::FrameIterator::parse(container.inner);
    assert_eq!(116676, frame_iter.count());

    let output = csdemo::parser::parse(csdemo::FrameIterator::parse(container.inner)).unwrap();

    assert_eq!("de_ancient", output.header.map_name());

    todo!()
}
