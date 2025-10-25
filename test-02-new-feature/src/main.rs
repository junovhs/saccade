mod plugin;
mod plugins;

use plugin::Plugin;
use plugins::{EchoPlugin, HelloPlugin};

fn main() {
    let plugins: Vec<Box<dyn Plugin>> = vec![
        Box::new(HelloPlugin),
        Box::new(EchoPlugin),
    ];

    for p in plugins {
        p.run("test");
    }
}
