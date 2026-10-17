//! Tools for looking into what the simulation did and why, which only read
//! the world: names for what they talk about in words a person reads, the
//! story a run's journal tells, what moved a point or a bone (the why), and
//! watchdogs that flag what makes no physical sense. `strike_scenarios`
//! prints them (`--journal`, `--why`, and the watchdogs on every run), and
//! the app's viewer (`--inspect`) shows them over the body. A digest of the
//! whole world tells two runs that should match apart.

mod blame;
mod digest;
mod names;
mod story;
mod watchdogs;
mod why;

pub use blame::{blame, blame_by_step, Blame, BlameTarget, PhasePush};
pub use digest::digest;
pub use names::{
    actor_name, bone_name, joint_name, measure_text, nearest_bone, place_name, subject_name,
    subject_spec, tool_name,
};
pub use story::{story, totals};
pub use watchdogs::{watchdog_lines, Trip, Watchdog, Watchdogs};
pub use why::{explain, target_name, Why};
