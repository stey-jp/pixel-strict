use pixelstrict_core::{MAX_INPUT_BYTES, Options, OutputMode, convert};
use std::{env, fs, path::Path, process};

fn run() -> Result<(), String> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "PixelStrict\nUsage: pixelstrict INPUT OUTPUT.png [--grid auto|WIDTH] [--output preserve|logical] [--colors auto|16|24|32] [--smoothing 1|2|3] [--edge 1|2|3] [--median] [--report FILE.json]\nOutput defaults to preserve: source dimensions with equal integer square cells. Grid WIDTH is the logical cell count across; its pitch must divide both source dimensions. Logical output keeps one pixel per cell. Existing files are never overwritten."
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
    if Path::new(&args[1]).exists() || report.as_ref().is_some_and(|p| Path::new(p).exists()) {
        return Err("Output/report already exists; choose new paths".into());
    }
    if report.as_ref().is_some_and(|p| p == &args[1]) {
        return Err("Output and report must be different files".into());
    }
    let metadata = fs::metadata(&args[0]).map_err(|e| e.to_string())?;
    if metadata.len() > MAX_INPUT_BYTES as u64 {
        return Err("Input exceeds 64 MiB".into());
    }
    let bytes = fs::read(&args[0]).map_err(|e| e.to_string())?;
    let converted = convert(&bytes, &options)?;
    use std::io::Write;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[1])
        .map_err(|e| e.to_string())?;
    output
        .write_all(&converted.png)
        .map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(&converted.report).map_err(|e| e.to_string())?;
    if let Some(path) = report {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        file.write_all(json.as_bytes()).map_err(|e| e.to_string())?;
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
