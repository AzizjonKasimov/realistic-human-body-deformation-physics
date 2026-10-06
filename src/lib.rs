mod mesh;
#[cfg(not(target_arch = "wasm32"))]
pub mod scenarios;
mod silhouette;
pub mod simulation;

pub use simulation::*;
