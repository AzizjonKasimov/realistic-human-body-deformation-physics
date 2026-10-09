mod mesh;
pub mod scenarios;
mod silhouette;
pub mod simulation;
pub mod sound;

pub use simulation::*;

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    /// Standard-library float math that natively calls the platform's C
    /// runtime (Microsoft's on Windows) but in the browser build a Rust port
    /// of musl's libm.
    const PLATFORM_MATH: &[&str] = &[
        "sin", "cos", "tan", "sin_cos", "asin", "acos", "atan", "atan2", "sinh", "cosh", "tanh",
        "asinh", "acosh", "atanh", "exp", "exp2", "exp_m1", "ln", "ln_1p", "log", "log2", "log10",
        "powf", "cbrt", "hypot",
    ];

    /// Modules under `src` that never feed the simulation, so they may use
    /// either: the binaries' own code (drawing and the native-only
    /// diagnostics) and the sound, which only reads the world.
    const NOT_SIMULATED: &[&str] = &["bin", "sound"];

    /// The browser build must simulate exactly like the native one, but the
    /// two runtimes differ in the last bit on many inputs (about a fifth of
    /// the bone angles' `atan2`s), which was enough to set
    /// `hammer_into_wound` apart. The simulation calls the `libm` crate, the
    /// same port, instead, so both builds compute alike.
    #[test]
    fn library_math_is_the_same_in_every_build() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        simulated_sources(&src, &mut files);
        let mut found = Vec::new();
        for file in &files {
            let text = std::fs::read_to_string(file).unwrap();
            for (index, line) in text.lines().enumerate() {
                let code = line.split("//").next().unwrap_or_default();
                for name in PLATFORM_MATH {
                    let calls = [format!(".{name}("), format!("f64::{name}(")];
                    if calls.iter().any(|call| code.contains(call.as_str())) {
                        let path = file.strip_prefix(&src).unwrap().to_string_lossy();
                        let path = path.replace('\\', "/");
                        found.push(format!("src/{path}:{}: {name}", index + 1));
                    }
                }
            }
        }
        assert!(
            found.is_empty(),
            "call libm, not the standard library's platform math, in code the simulation runs \
             (a module that never feeds it belongs in NOT_SIMULATED):\n{}",
            found.join("\n")
        );
    }

    /// Every source file under `dir` outside the modules in `NOT_SIMULATED`.
    fn simulated_sources(dir: &Path, files: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let module = path.file_stem().and_then(|stem| stem.to_str());
            if module.is_some_and(|module| NOT_SIMULATED.contains(&module)) {
                continue;
            }
            if path.is_dir() {
                simulated_sources(&path, files);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                files.push(path);
            }
        }
    }
}
