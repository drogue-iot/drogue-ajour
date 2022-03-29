#![recursion_limit = "1024"]
#![allow(clippy::needless_return)]

extern crate core;

mod app;
mod connector;
mod data;
mod pages;
mod settings;
mod simulator;
mod utils;

use crate::app::Application;
use wasm_bindgen::prelude::*;

#[cfg(not(debug_assertions))]
const LOG_LEVEL: log::Level = log::Level::Info;
#[cfg(debug_assertions)]
const LOG_LEVEL: log::Level = log::Level::Trace;

pub fn main() -> Result<(), JsValue> {
    wasm_logger::init(wasm_logger::Config::new(LOG_LEVEL));
    log::info!("Getting ready...");
    yew::start_app::<Application>();
    Ok(())
}
