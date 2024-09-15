#[test]
fn mirage_1() {
    let content = std::fs::read("testfiles/mirage.dem").unwrap();

    let container = csdemo::Container::parse(&content).unwrap();

    let frame_iter = csdemo::FrameIterator::parse(container.inner);
    assert_eq!(123333, frame_iter.count());

    let output = csdemo::parser::parse(csdemo::FrameIterator::parse(container.inner)).unwrap();

    assert_eq!("de_mirage", output.header.map_name());

    todo!()
}
