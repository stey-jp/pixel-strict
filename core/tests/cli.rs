use std::{fs, process::Command, time::SystemTime};

#[test]
fn cli_output_modes_reports_and_errors() {
    let binary = env!("CARGO_BIN_EXE_pixelstrict");
    let input =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../samples/house-pseudo.png");
    let temp = std::env::temp_dir().join(format!(
        "pixelstrict-cli-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&temp).unwrap();
    let help = Command::new(binary).arg("--help").output().unwrap();
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("--output preserve|logical"));
    for (mode, dimension) in [("default", 192), ("preserve", 192), ("logical", 32)] {
        let output = temp.join(format!("{mode}.png"));
        let report = temp.join(format!("{mode}.json"));
        let mut cmd = Command::new(binary);
        cmd.arg(&input)
            .arg(&output)
            .args([
                "--grid",
                "32",
                "--colors",
                "24",
                "--smoothing",
                "3",
                "--edge",
                "2",
                "--median",
                "--report",
            ])
            .arg(&report);
        if mode != "default" {
            cmd.args(["--output", mode]);
        }
        let result = cmd.output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            image::image_dimensions(&output).unwrap(),
            (dimension, dimension)
        );
        let json: serde_json::Value = serde_json::from_slice(&fs::read(report).unwrap()).unwrap();
        assert_eq!(
            json["output_mode"],
            if mode == "logical" {
                "logical"
            } else {
                "preserve"
            }
        );
        assert_eq!(json["output_width"], dimension);
        assert_eq!(json["output_height"], dimension);
        assert_eq!(json["grid"]["width"], 32);
        assert_eq!(json["cell_pitch"], if mode == "logical" { 1 } else { 6 });
        let original = fs::read(&output).unwrap();
        let duplicate = cmd.output().unwrap();
        assert!(!duplicate.status.success());
        assert!(String::from_utf8_lossy(&duplicate.stderr).contains("already exists"));
        assert_eq!(fs::read(output).unwrap(), original);
    }
    let output = temp.join("invalid.png");
    for args in [
        vec!["--output", "invalid"],
        vec!["--output"],
        vec!["--grid", "31", "--output", "preserve"],
    ] {
        let result = Command::new(binary)
            .arg(&input)
            .arg(&output)
            .args(args)
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(!output.exists());
    }
    fs::remove_dir_all(temp).unwrap();
}
