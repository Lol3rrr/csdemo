#[test]
fn cmp_lazy_nonlazy_events() {
    let content = std::fs::read("testfiles/mirage.dem").unwrap();

    let container = csdemo::Container::parse(&content).unwrap();
    let demo = csdemo::parser::parse(
        csdemo::FrameIterator::parse(container.inner),
        csdemo::parser::EntityFilter::disabled(),
    )
    .unwrap();

    let lazy_demo =
        csdemo::lazyparser::LazyParser::new(csdemo::Container::parse(&content).unwrap());

    assert_eq!(demo.player_info, lazy_demo.player_info());

    for (normal, lazied) in demo
        .events
        .into_iter()
        .zip(lazy_demo.events().filter_map(|e| e.ok()))
    {
        assert_eq!(normal, lazied);
    }
}

#[test]
fn cmp_lazy_nonlazy_entities() {
    let content = std::fs::read("testfiles/mirage.dem").unwrap();

    let container = csdemo::Container::parse(&content).unwrap();
    let demo = csdemo::parser::parse(
        csdemo::FrameIterator::parse(container.inner),
        csdemo::parser::EntityFilter::all(),
    )
    .unwrap();

    let lazy_demo =
        csdemo::lazyparser::LazyParser::new(csdemo::Container::parse(&content).unwrap());

    assert_eq!(demo.player_info, lazy_demo.player_info());

    let mut normal_iter = demo
        .entity_states
        .ticks
        .into_iter()
        .flat_map(|t| t.states.into_iter().map(move |s| (t.tick, s)));
    let mut lazy_iter = lazy_demo.entities().filter_map(|e| e.ok());

    while let Some(normal) = normal_iter.next() {
        let lazy = lazy_iter.next().unwrap();

        assert_eq!(normal, lazy);
    }
    assert_eq!(None, lazy_iter.next());
}
