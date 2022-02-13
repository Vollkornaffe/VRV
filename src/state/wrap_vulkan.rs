pub struct State {}

impl State {
    pub fn new() -> State {
        log::info!("Creating new Vulkan State");
        Self {}
    }
}
