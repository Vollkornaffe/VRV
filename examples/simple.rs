use simplelog::{Config, SimpleLogger};
use vrv::State;

fn main() {
    let _ = SimpleLogger::init(log::LevelFilter::Debug, Config::default());
    let _state = State::new().map_err(|e| eprintln!("State creation failed with: {:?}", e));
}
