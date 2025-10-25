use crate::plugin::Plugin;

pub struct HelloPlugin;

impl Plugin for HelloPlugin {
    fn name(&self) -> &'static str {
        "hello"
    }
    fn run(&self, _input: &str) {
        println!("[hello] Hello, world!");
    }
}
