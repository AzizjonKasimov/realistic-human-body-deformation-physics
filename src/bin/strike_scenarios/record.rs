//! A run recorded for the Rerun viewer (`--record FILE.rrd`, built with the
//! `record` feature; `tools\record.ps1` builds, records, and opens it): every
//! point of flesh, fiber, bone, joint, the tool, and the blood on a timeline
//! of frames; what the injury rules decided and what the watchdogs flagged as
//! a log on the same timeline; and the numbers that tell whether the body is
//! where it should be, plotted over the run: each fist's knock, the head's
//! turn, each joint's slip and turn, each bone's load, and how far each
//! bone's flesh has slid off it. Scrub the timeline to a frame, click
//! anything to see what it is, and set two recordings side by side to see
//! what a change did.

use super::*;

/// A run being recorded, or, in a build without the `record` feature,
/// nothing.
pub(crate) struct Recorder {
    #[cfg(feature = "record")]
    inner: inner::Inner,
}

impl Recorder {
    /// Starts recording the run of `name` on `world` to `path`.
    pub(crate) fn start(path: &Path, name: &str, world: &rp::World) -> Option<Recorder> {
        #[cfg(feature = "record")]
        {
            match inner::Inner::start(path, name, world) {
                Ok(inner) => Some(Recorder { inner }),
                Err(error) => {
                    eprintln!("cannot record to {}: {error}", path.display());
                    None
                }
            }
        }
        #[cfg(not(feature = "record"))]
        {
            let _ = (name, world);
            eprintln!(
                "this build cannot record {}: build with `--features record` (tools\\record.ps1 does)",
                path.display()
            );
            None
        }
    }

    /// Records `frame`, with the watchdogs' trips so far.
    pub(crate) fn frame(&mut self, frame: i32, world: &rp::World, trips: &[rp::inspect::Trip]) {
        #[cfg(feature = "record")]
        self.inner.frame(frame, world, trips);
        #[cfg(not(feature = "record"))]
        let _ = (frame, world, trips);
    }

    /// Ends the recording and says where it is.
    pub(crate) fn finish(self) {
        #[cfg(feature = "record")]
        self.inner.finish();
    }
}

#[cfg(feature = "record")]
mod inner {
    use super::*;
    use rerun::{
        Color, LineStrips2D, Points2D, RecordingStream, RecordingStreamBuilder, Scalars,
        SeriesLines, TextLog, TextLogLevel,
    };

    pub(super) struct Inner {
        stream: RecordingStream,
        path: PathBuf,
        /// Where each arm's fist stood when the play began.
        fists_at_start: [rp::Vec2; 2],
        /// The watchdog trips already logged.
        trips_logged: usize,
        /// The journal events already logged.
        events_logged: usize,
        last_positions: Vec<rp::Vec2>,
        dt: f64,
        /// How many joints and bones the body was built with, which the
        /// plots follow.
        joints: usize,
        bones: usize,
    }

    fn xy(point: rp::Vec2) -> [f32; 2] {
        [point.x as f32, point.y as f32]
    }

    /// A colour from blue (squeezed) through grey to yellow and red
    /// (stretched toward tearing) for a fiber `strain` past its rest.
    fn strain_colour(strain: f64) -> Color {
        let s = (strain / 0.25).clamp(-1.0, 1.0);
        if s < 0.0 {
            let t = -s;
            Color::from_rgb(
                (150.0 * (1.0 - t)) as u8,
                (150.0 * (1.0 - t) + 80.0 * t) as u8,
                (150.0 + 105.0 * t) as u8,
            )
        } else if s < 0.5 {
            let t = s * 2.0;
            Color::from_rgb(
                (150.0 + 105.0 * t) as u8,
                (150.0 + 70.0 * t) as u8,
                (150.0 * (1.0 - t)) as u8,
            )
        } else {
            let t = (s - 0.5) * 2.0;
            Color::from_rgb(255, (220.0 * (1.0 - t)) as u8, 0)
        }
    }

    impl Inner {
        pub(super) fn start(path: &Path, name: &str, world: &rp::World) -> Result<Inner, String> {
            let stream = RecordingStreamBuilder::new("realistic_physics")
                .recording_name(name)
                .save(path)
                .map_err(|error| error.to_string())?;
            // What the body was built as, faint, to see how far it went.
            let built: Vec<Vec<[f32; 2]>> = world
                .bones()
                .iter()
                .map(|bone| vec![xy(bone.home_a), xy(bone.home_b)])
                .collect();
            stream
                .log_static(
                    "built/bones",
                    &LineStrips2D::new(built)
                        .with_colors([Color::from_unmultiplied_rgba(255, 255, 255, 40)])
                        .with_radii([1.0f32]),
                )
                .map_err(|error| error.to_string())?;
            // The floor under the body, as wide as the body is tall, so the
            // view frames the body rather than the window.
            let floor = world.floor_y() as f32;
            let body = rp::body_frame(SCENARIO_WIDTH, SCENARIO_HEIGHT);
            let (left, right) = (
                body.point(-0.5, 0.0).x as f32,
                body.point(0.5, 0.0).x as f32,
            );
            if floor.is_finite() {
                stream
                    .log_static(
                        "built/floor",
                        &LineStrips2D::new([vec![[left, floor], [right, floor]]])
                            .with_colors([Color::from_rgb(90, 80, 70)]),
                    )
                    .map_err(|error| error.to_string())?;
            }
            Inner::series_names(&stream, world)?;
            Ok(Inner {
                stream,
                path: path.to_path_buf(),
                fists_at_start: [rp::Vec2::default(); 2],
                trips_logged: 0,
                events_logged: 0,
                last_positions: Vec::new(),
                dt: world.materials().fixed_dt,
                joints: world.bone_joints().len(),
                bones: world.bones().len(),
            })
        }

        pub(super) fn frame(&mut self, frame: i32, world: &rp::World, trips: &[rp::inspect::Trip]) {
            if frame == 0 {
                self.fists_at_start = rp::scenarios::fists_off_their_place(world);
            }
            self.stream.set_time_sequence("frame", i64::from(frame));
            self.stream
                .set_duration_secs("time", f64::from(frame) * self.dt);
            // Results of logging are dropped: a recording missing a frame
            // is still worth reading, and the file sink reports its own
            // failures when it closes.
            self.log_story(world, trips);
            let log = |path: &str, data: &dyn rerun::AsComponents| {
                let _ = self.stream.log(path, data);
            };
            self.log_flesh(world, &log);
            self.log_skeleton(world, &log);
            self.log_tool_and_blood(world, &log);
            self.log_plots(world, &log);
            self.last_positions = world.points().iter().map(|point| point.position).collect();
        }

        fn log_flesh(&self, world: &rp::World, log: &dyn Fn(&str, &dyn rerun::AsComponents)) {
            let points = world.points();
            for (layer, name, colour) in [
                (
                    rp::TissueLayer::Skin,
                    "skin",
                    Color::from_rgb(214, 160, 130),
                ),
                (
                    rp::TissueLayer::Muscle,
                    "muscle",
                    Color::from_rgb(150, 40, 45),
                ),
            ] {
                let mine: Vec<&rp::Point> = points
                    .iter()
                    .filter(|point| point.layer == layer && !point.is_ghost())
                    .collect();
                let colours: Vec<Color> = mine
                    .iter()
                    .map(|point| {
                        let bruise = point.bruise_shown.clamp(0.0, 1.0);
                        let [r, g, b, _] = colour.to_array();
                        Color::from_rgb(
                            (f64::from(r) * (1.0 - bruise) + 90.0 * bruise) as u8,
                            (f64::from(g) * (1.0 - bruise) + 30.0 * bruise) as u8,
                            (f64::from(b) * (1.0 - bruise) + 70.0 * bruise) as u8,
                        )
                    })
                    .collect();
                log(
                    &format!("body/{name}/points"),
                    &Points2D::new(mine.iter().map(|point| xy(point.position)))
                        .with_colors(colours)
                        .with_radii([1.6f32]),
                );
                let fibers: Vec<(&rp::Spring, &rp::SpringWear)> = world
                    .springs()
                    .iter()
                    .zip(world.spring_wear())
                    .filter(|(spring, _)| {
                        !spring.broken
                            && points
                                .get(spring.a)
                                .is_some_and(|point| point.layer == layer)
                    })
                    .collect();
                let strains: Vec<Color> = fibers
                    .iter()
                    .map(|(spring, wear)| {
                        let (a, b) = (points[spring.a].position, points[spring.b].position);
                        let length = (b.x - a.x).hypot(b.y - a.y);
                        strain_colour(length / spring.rest.max(1e-6) - 1.0 - wear.prestretch)
                    })
                    .collect();
                log(
                    &format!("body/{name}/fibers"),
                    &LineStrips2D::new(fibers.iter().map(|(spring, _)| {
                        vec![xy(points[spring.a].position), xy(points[spring.b].position)]
                    }))
                    .with_colors(strains)
                    .with_radii([0.35f32]),
                );
            }
        }

        fn log_skeleton(&self, world: &rp::World, log: &dyn Fn(&str, &dyn rerun::AsComponents)) {
            let bones = world.bones();
            log(
                "body/bones",
                &LineStrips2D::new(bones.iter().map(|bone| vec![xy(bone.a), xy(bone.b)]))
                    .with_radii(bones.iter().map(|bone| bone.radius as f32))
                    .with_colors(bones.iter().map(|bone| {
                        if bone.splinter {
                            Color::from_rgb(240, 150, 60)
                        } else if bone.fractured {
                            Color::from_rgb(230, 90, 80)
                        } else {
                            Color::from_rgb(235, 225, 200)
                        }
                    }))
                    .with_labels(
                        (0..bones.len()).map(|index| {
                            format!("{index} {}", rp::inspect::bone_name(world, index))
                        }),
                    )
                    // Named on hover, not over the body.
                    .with_show_labels(false),
            );
            let joints = world.bone_joints();
            let anchors: Vec<[f32; 2]> = joints
                .iter()
                .map(|joint| {
                    bones.get(joint.b).map_or([0.0, 0.0], |bone| {
                        xy(rp::Vec2 {
                            x: bone.a.x + (bone.b.x - bone.a.x) * joint.t_b,
                            y: bone.a.y + (bone.b.y - bone.a.y) * joint.t_b,
                        })
                    })
                })
                .collect();
            log(
                "body/joints",
                &Points2D::new(anchors)
                    .with_radii([3.0f32])
                    .with_colors(joints.iter().map(|joint| {
                        if joint.broken {
                            Color::from_rgb(255, 40, 40)
                        } else if joint.post_fracture_limited {
                            Color::from_rgb(255, 140, 0)
                        } else if joint.subluxated {
                            Color::from_rgb(255, 220, 0)
                        } else {
                            Color::from_rgb(90, 200, 255)
                        }
                    }))
                    .with_labels(
                        (0..joints.len()).map(|index| {
                            format!("{index} {}", rp::inspect::joint_name(world, index))
                        }),
                    )
                    .with_show_labels(false),
            );
        }

        fn log_tool_and_blood(
            &self,
            world: &rp::World,
            log: &dyn Fn(&str, &dyn rerun::AsComponents),
        ) {
            if let Some(pose) = world.current_tool_pose() {
                log(
                    "tool",
                    &LineStrips2D::new([vec![xy(pose.contact_start), xy(pose.contact_end)]])
                        .with_radii([pose.contact_radius as f32])
                        .with_colors([Color::from_unmultiplied_rgba(200, 200, 210, 160)])
                        .with_labels([rp::scenarios::tool_name(pose.tool)]),
                );
            }
            let drops = world.fluids();
            log(
                "blood",
                &Points2D::new(drops.iter().map(|drop| xy(drop.position)))
                    .with_colors([Color::from_rgb(170, 0, 10)])
                    .with_radii(drops.iter().map(|drop| world.drop_radius(drop) as f32)),
            );
        }

        fn log_story(&mut self, world: &rp::World, trips: &[rp::inspect::Trip]) {
            if let Some(journal) = world.journal() {
                for event in &journal.events[self.events_logged..] {
                    let level = if event.kind.is_minor() {
                        TextLogLevel::DEBUG
                    } else {
                        TextLogLevel::INFO
                    };
                    let line = format!(
                        "{} at {} ({} phase)",
                        event.kind.describe(),
                        rp::inspect::subject_name(world, event.subject),
                        event.phase
                    );
                    let _ = self
                        .stream
                        .log("story/injuries", &TextLog::new(line).with_level(level));
                }
                self.events_logged = journal.events.len();
            }
            for trip in &trips[self.trips_logged.min(trips.len())..] {
                let line = format!(
                    "watchdog: {}: {} from frame {}",
                    trip.watchdog.describe(),
                    rp::inspect::subject_name(world, trip.subject),
                    trip.first_frame
                );
                let _ = self.stream.log(
                    "story/watchdogs",
                    &TextLog::new(line).with_level(TextLogLevel::WARN),
                );
            }
            self.trips_logged = trips.len();
        }

        fn log_plots(&self, world: &rp::World, log: &dyn Fn(&str, &dyn rerun::AsComponents)) {
            let at_start = self.fists_at_start;
            let now = rp::scenarios::fists_off_their_place(world);
            let knocks: Vec<f64> = (0..2)
                .map(|arm| (now[arm].x - at_start[arm].x).hypot(now[arm].y - at_start[arm].y))
                .collect();
            log("plots/fist_knocked_px", &Scalars::new(knocks));
            log(
                "plots/head_turn_degrees",
                &Scalars::new([rp::scenarios::head_turn_degrees(world)]),
            );
            let fastest = world
                .points()
                .iter()
                .zip(&self.last_positions)
                .map(|(point, last)| {
                    (point.position.x - last.x).hypot(point.position.y - last.y) / self.dt
                })
                .fold(0.0, f64::max);
            log("plots/fastest_tissue_px_s", &Scalars::new([fastest]));
            log(
                "plots/blood_lost_ml",
                &Scalars::new([world.stats().blood_loss * rp::scenarios::BLOOD_VOLUME_ML]),
            );
            // The joints and bones the body was built with, each a named
            // series of one plot (`series_names`); one torn or broken leaves
            // a gap, and the pieces a break makes are not plotted.
            let joints = 0..self.joints.min(world.bone_joints().len());
            let slips: Vec<f64> = joints
                .clone()
                .map(|index| world.joint_slip(index).unwrap_or(f64::NAN))
                .collect();
            log("plots/joint_slip_px", &Scalars::new(slips));
            let turns: Vec<f64> = joints
                .map(|index| world.joint_turn(index).map_or(f64::NAN, f64::to_degrees))
                .collect();
            log("plots/joint_turn_degrees", &Scalars::new(turns));
            let bones = world.bones();
            let mut slid = vec![(0.0f64, 0usize); bones.len()];
            for tie in world.bone_attachments() {
                if !tie.broken && tie.bone < bones.len() && tie.point < world.points().len() {
                    slid[tie.bone].0 += world.bone_tie_slip(tie);
                    slid[tie.bone].1 += 1;
                }
            }
            let built = 0..self.bones.min(bones.len());
            let loads: Vec<f64> = built
                .clone()
                .map(|index| bones[index].break_load / bones[index].fracture_impulse.max(1.0))
                .collect();
            log("plots/bone_load", &Scalars::new(loads));
            let slides: Vec<f64> = built
                .map(|index| {
                    if slid[index].1 > 0 {
                        slid[index].0 / slid[index].1 as f64
                    } else {
                        f64::NAN
                    }
                })
                .collect();
            log("plots/flesh_slid_px", &Scalars::new(slides));
        }

        /// Names each series of the plots that hold one a joint or a bone,
        /// once, for the whole recording.
        fn series_names(stream: &RecordingStream, world: &rp::World) -> Result<(), String> {
            let joints: Vec<String> = (0..world.bone_joints().len())
                .map(|index| format!("{index} {}", rp::inspect::joint_name(world, index)))
                .collect();
            let bones: Vec<String> = (0..world.bones().len())
                .map(|index| format!("{index} {}", rp::inspect::bone_name(world, index)))
                .collect();
            for (path, names) in [
                (
                    "plots/fist_knocked_px",
                    vec!["left".to_string(), "right".to_string()],
                ),
                ("plots/joint_slip_px", joints.clone()),
                ("plots/joint_turn_degrees", joints),
                ("plots/bone_load", bones.clone()),
                ("plots/flesh_slid_px", bones),
            ] {
                stream
                    .log_static(path, &SeriesLines::new().with_names(names))
                    .map_err(|error| error.to_string())?;
            }
            Ok(())
        }

        pub(super) fn finish(self) {
            let _ = self.stream.flush_blocking();
            println!(
                "recorded {} (open it with `rerun {}`)",
                self.path.display(),
                self.path.display()
            );
        }
    }
}
