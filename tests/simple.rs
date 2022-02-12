use simplelog::{Config, SimpleLogger};
use vrv::State;

#[test]
fn run() {
    let _ = SimpleLogger::init(log::LevelFilter::Trace, Config::default());
    let _state = State::new();
}
