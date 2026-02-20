use anyhow::Result;
use std::path::Path;
use std::process::Command;

pub fn get_sample_logs() -> Result<String> {
    let log_file_name = "examples/data/SSH.log";
    let compressed_file_name = "examples/data/SSH.tar.gz";

    if !Path::new(log_file_name).exists() {
        if !Path::new(compressed_file_name).exists() {
            println!("Downloading {}...", compressed_file_name);
            let status = Command::new("curl")
                .arg("-L")
                .arg("https://zenodo.org/record/3227177/files/SSH.tar.gz")
                .arg("--output")
                .arg(compressed_file_name)
                .status()?;

            if !status.success() {
                return Err(anyhow::anyhow!("failed to download file"));
            }
        }

        println!("Extracting {}...", compressed_file_name);
        let status = Command::new("tar")
            .arg("-xvzf")
            .arg(compressed_file_name)
            .status()?;

        if !status.success() {
            return Err(anyhow::anyhow!("Failed to extract file."));
        }
    }

    Ok(log_file_name.to_string())
}
