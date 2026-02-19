use drain3::config::TemplateMinerConfig;
use drain3::drain::LogCluster;
use drain3::TemplateMiner;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::Command;
use std::time::Instant;

fn main() -> anyhow::Result<()> {
    // Load config if exists
    // Load config from examples directory since we run from crate root
    let config_path = "examples/drain3.toml";
    let config = if Path::new(config_path).exists() {
        TemplateMinerConfig::load(config_path).unwrap_or_default()
    } else {
        eprintln!("Config file not found at {}, using defaults", config_path);
        TemplateMinerConfig::default()
    };

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
                eprintln!("Failed to download file");
                std::process::exit(1);
            }
        }

        println!("Extracting {}...", compressed_file_name);
        let status = Command::new("tar")
            .arg("-xvzf")
            .arg(compressed_file_name)
            .status()?;

        if !status.success() {
            eprintln!("Failed to extract file. Deleting and retrying...");
            std::fs::remove_file(compressed_file_name)?;

            // Retry download once
            println!("Downloading {}...", compressed_file_name);
            let status = Command::new("curl")
                .arg("-L")
                .arg("https://zenodo.org/record/3227177/files/SSH.tar.gz")
                .arg("--output")
                .arg(compressed_file_name)
                .status()?;

            if !status.success() {
                eprintln!("Failed to download file");
                std::process::exit(1);
            }

            println!("Extracting {}...", compressed_file_name);
            let status = Command::new("tar")
                .arg("-xvzf")
                .arg(compressed_file_name)
                .status()?;

            if !status.success() {
                eprintln!("Failed to extract file after retry.");
                std::process::exit(1);
            }
        }
    }

    let mut miner = TemplateMiner::new(config, None);

    let file = File::open(log_file_name)?;
    let reader = BufReader::new(file);

    let start = Instant::now();
    let mut batch_start = start;
    let mut line_count = 0;

    let output_path = "examples/outputs/drain3_output.csv";
    let mut output_file = File::create(output_path)?;
    writeln!(output_file, "template_id,size,template")?;

    println!("Processing {}...", log_file_name);

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Python demo strips header like "December 10 07:07:38 labszhu-5 " logic
        // We will just process the whole line for now or maybe strip known header?
        // The python demo does: line = line.partition(": ")[2]
        // Let's replicate this simple heuristic if it splits by first ": "
        let content = if let Some(idx) = line.find(": ") {
            &line[idx + 2..]
        } else {
            line
        };

        let (cluster, change_type) = miner.add_log_message(content);

        line_count += 1;
        if line_count % 10000 == 0 {
            let now = Instant::now();
            let batch_duration = now.duration_since(batch_start);
            let batch_lines_sec = 10000.0 / batch_duration.as_secs_f64();
            println!(
                "Processing line: {}, rate {:.1} lines/sec, {} clusters so far.",
                line_count,
                batch_lines_sec,
                miner.drain.id_to_cluster.len()
            );
            batch_start = now;
        }
    }

    let duration = start.elapsed();
    let lines_per_sec = if duration.as_secs_f64() > 0.0 {
        line_count as f64 / duration.as_secs_f64()
    } else {
        0.0
    };

    println!(
        "--- Done processing file in {:.2?} sec. Total of {} lines, rate {:.1} lines/sec, {} clusters", 
        duration,
        line_count,
        lines_per_sec,
        miner.drain.id_to_cluster.len()
    );

    let mut clusters: Vec<&LogCluster> = miner.drain.id_to_cluster.values().collect();
    clusters.sort_by_key(|c| c.cluster_id);

    for cluster in clusters {
        writeln!(
            output_file,
            "{},{},\"{}\"",
            cluster.cluster_id,
            cluster.size,
            cluster.get_template().replace("\"", "\"\"")
        )?;
    }

    Ok(())
}
