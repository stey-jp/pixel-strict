use pixelstrict_core::{MAX_INPUT_BYTES, Options, OutputMode, convert};
use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process,
};

// Resolve existing parents too, so ./, .. and directory links cannot hide a
// collision between two new destinations. create_new remains the race guard.
fn destination(path: &Path) -> Result<PathBuf, String> {
    let name = path.file_name().ok_or("Output/report must name a file")?;
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
    let parent = fs::canonicalize(parent.unwrap_or(Path::new(".")))
        .map_err(|e| format!("Invalid output/report directory: {e}"))?;
    Ok(parent.join(name))
}

struct PendingFile {
    path: PathBuf,
    file: Option<fs::File>,
    keep: bool,
}

impl PendingFile {
    fn create(path: PathBuf) -> Result<Self, String> {
        let file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        Ok(Self {
            path,
            file: Some(file),
            keep: false,
        })
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        let file = self.file.as_mut().unwrap();
        file.write_all(bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())
    }
}

impl Drop for PendingFile {
    fn drop(&mut self) {
        // Close before removal on Windows; never remove a pre-existing file.
        drop(self.file.take());
        if !self.keep {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "PixelStrict\nUsage: pixelstrict INPUT OUTPUT.png [--grid auto|WIDTH] [--output preserve|logical] [--colors auto|16|24|32] [--smoothing 1|2|3] [--edge 1|2|3] [--shape 1|2|3] [--median] [--report FILE.json]\nOutput defaults to preserve: source dimensions with equal integer square cells. Grid WIDTH is the logical cell count across; its pitch must divide both source dimensions. Logical output keeps one pixel per cell. Existing files are never overwritten."
        );
        return Ok(());
    }
    if args.len() < 2 {
        return Err("Usage: pixelstrict INPUT OUTPUT.png [options]; use --help".into());
    }
    let mut options = Options::default();
    let mut report = None;
    let mut i = 2;
    while i < args.len() {
        let flag = args[i].to_str().ok_or("Invalid flag")?;
        if flag == "--median" {
            options.median = true;
            i += 1;
            continue;
        }
        i += 1;
        let value = args
            .get(i)
            .ok_or_else(|| format!("Missing value for {flag}"))?;
        let number = || {
            value
                .to_str()
                .unwrap_or("")
                .parse::<u32>()
                .map_err(|_| format!("Invalid value for {flag}"))
        };
        match flag {
            "--output" => {
                options.output_mode = match value.to_str() {
                    Some("preserve") => OutputMode::Preserve,
                    Some("logical") => OutputMode::Logical,
                    _ => return Err("Output must be preserve or logical".into()),
                }
            }
            "--grid" => {
                options.grid_width = if value == "auto" {
                    None
                } else {
                    Some(number()?)
                }
            }
            "--colors" => {
                options.colors = if value == "auto" {
                    None
                } else {
                    Some(u8::try_from(number()?).map_err(|_| "Colors out of range")?)
                }
            }
            "--smoothing" => {
                options.smoothing = u8::try_from(number()?).map_err(|_| "Strength out of range")?
            }
            "--edge" => {
                options.edge_protection =
                    u8::try_from(number()?).map_err(|_| "Strength out of range")?
            }
            "--report" => report = Some(value.clone()),
            "--shape" => {
                options.shape_protection =
                    u8::try_from(number()?).map_err(|_| "Strength out of range")?
            }
            _ => return Err(format!("Unknown option {flag}")),
        }
        i += 1;
    }
    if !Path::new(&args[1])
        .extension()
        .is_some_and(|s| s.eq_ignore_ascii_case("png"))
    {
        return Err("Output extension must be .png".into());
    }
    let output_path = destination(Path::new(&args[1]))?;
    let report_path = report
        .as_ref()
        .map(|p| destination(Path::new(p)))
        .transpose()?;
    if output_path.exists() || report_path.as_ref().is_some_and(|p| p.exists()) {
        return Err("Output/report already exists; choose new paths".into());
    }
    if report_path.as_ref().is_some_and(|p| {
        p == &output_path
            || (cfg!(windows)
                && p.to_string_lossy()
                    .eq_ignore_ascii_case(&output_path.to_string_lossy()))
    }) {
        return Err("Output and report must be different files".into());
    }
    let metadata = fs::metadata(&args[0]).map_err(|e| e.to_string())?;
    if metadata.len() > MAX_INPUT_BYTES as u64 {
        return Err("Input exceeds 64 MiB".into());
    }
    let bytes = fs::read(&args[0]).map_err(|e| e.to_string())?;
    let converted = convert(&bytes, &options)?;
    let json = serde_json::to_string_pretty(&converted.report).map_err(|e| e.to_string())?;
    let mut output = PendingFile::create(output_path)?;
    let mut report = report_path.map(PendingFile::create).transpose()?;
    output.write(&converted.png)?;
    if let Some(file) = &mut report {
        file.write(json.as_bytes())?;
    }
    output.keep = true;
    if let Some(file) = &mut report {
        file.keep = true;
    }
    println!("{json}");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("PixelStrict: {e}");
        process::exit(1);
    }
}
