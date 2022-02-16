use anyhow::Result;
use std::mem::ManuallyDrop;

pub mod wrap_window;
pub mod wrap_openxr;
pub mod wrap_vulkan;

pub struct State {
    pub window: ManuallyDrop<wrap_window::State>, 
    pub openxr: ManuallyDrop<wrap_openxr::State>,
    pub vulkan: ManuallyDrop<wrap_vulkan::base::Base>,
}

impl Drop for State {
    fn drop(&mut self) {
        unsafe{
            ManuallyDrop::drop(&mut self.vulkan);
            ManuallyDrop::drop(&mut self.openxr);
            ManuallyDrop::drop(&mut self.window);
        }
    }
}

impl State {

    pub fn new() -> Result <Self> {
        let window = wrap_window::State::new();
        let openxr = wrap_openxr::State::new()?;
        let vulkan = wrap_vulkan::Base::new(&window, &openxr)?;
        Ok(Self{
            window: ManuallyDrop::new(window),
            openxr: ManuallyDrop::new(openxr),
            vulkan: ManuallyDrop::new(vulkan),
        })
    } 
}