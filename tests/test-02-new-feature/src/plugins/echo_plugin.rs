use crate::plugin::Plugin;

pub struct EchoPlugin;

impl Plugin for EchoPlugin {
    fn name(&self) -> &'static str {
        "echo"
    }
    fn run(&self, input: &str) {
        println!("[echo] {}", input);
    }
}

// Module declaration for plugins
mod lib;
