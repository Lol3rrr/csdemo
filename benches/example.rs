#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

mod eager {
    #[divan::bench(max_time = std::time::Duration::from_secs(30))]
    fn no_entities_mirage() -> csdemo::parser::FirstPassOutput {
        let raw_bytes = include_bytes!("../testfiles/mirage.dem");

        let container = csdemo::Container::parse(divan::black_box(raw_bytes.as_slice())).unwrap();

        csdemo::parser::parse(
            csdemo::FrameIterator::parse(container.inner),
            csdemo::parser::EntityFilter::disabled(),
        )
        .unwrap()
    }

    #[divan::bench(max_time = std::time::Duration::from_secs(30))]
    fn entities_mirage() -> csdemo::parser::FirstPassOutput {
        let raw_bytes = include_bytes!("../testfiles/mirage.dem");

        let container = csdemo::Container::parse(divan::black_box(raw_bytes.as_slice())).unwrap();

        csdemo::parser::parse(
            csdemo::FrameIterator::parse(container.inner),
            csdemo::parser::EntityFilter::all(),
        )
        .unwrap()
    }
}
