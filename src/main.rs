mod application;
mod config;
mod dialog;
mod directory;
mod error;
mod images;
mod renderer;

use application::*;

fn main() {
    wita::initialize::<Application>();
    wita::run(wita::RunType::Wait, Application::new().unwrap());
}
