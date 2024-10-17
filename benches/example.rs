#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

mod eager {
    #[divan::bench(max_time = std::time::Duration::from_secs(30))]
    fn no_entities_mirage() {
        let raw_bytes = include_bytes!("../testfiles/mirage.dem");

        let container = csdemo::Container::parse(divan::black_box(raw_bytes.as_slice())).unwrap();

        let demo = csdemo::parser::parse(
            csdemo::FrameIterator::parse(container.inner),
            csdemo::parser::EntityFilter::disabled(),
        )
        .unwrap();

        for event in demo.events {
            divan::black_box(event);
        }
    }

    #[divan::bench(max_time = std::time::Duration::from_secs(30))]
    fn entities_mirage() {
        let raw_bytes = include_bytes!("../testfiles/mirage.dem");

        let container = csdemo::Container::parse(divan::black_box(raw_bytes.as_slice())).unwrap();

        let demo = csdemo::parser::parse(
            csdemo::FrameIterator::parse(container.inner),
            csdemo::parser::EntityFilter::all(),
        )
        .unwrap();

        for event in demo.events {
            divan::black_box(event);
        }
        for entity in demo.entity_states.ticks {
            divan::black_box(entity);
        }
    }
}

mod lazy {
    #[divan::bench(max_time = std::time::Duration::from_secs(30))]
    fn no_entities_mirage() {
        let raw_bytes = include_bytes!("../testfiles/mirage.dem");

        let container = csdemo::Container::parse(divan::black_box(raw_bytes.as_slice())).unwrap();

        let demo = csdemo::lazyparser::LazyParser::new(container);

        for event in demo.events() {
            divan::black_box(event);
        }
    }

    #[divan::bench(max_time = std::time::Duration::from_secs(30))]
    fn entities_mirage() {
        let raw_bytes = include_bytes!("../testfiles/mirage.dem");

        let container = csdemo::Container::parse(divan::black_box(raw_bytes.as_slice())).unwrap();

        let demo = csdemo::lazyparser::LazyParser::new(container);

        for event in demo.events() {
            divan::black_box(event);
        }
        for entity in demo.entities() {
            divan::black_box(entity);
        }
    }
}
