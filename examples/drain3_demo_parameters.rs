use drain3::config::TemplateMinerConfig;

use drain3::file_persistence::FilePersistence;
use drain3::template_miner::TemplateMiner;
use drain3::{LogCluster, SearchStrategy};

use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::time::Instant;

mod sample_logs;

fn main() -> anyhow::Result<()> {
    // Load config if exists
    // Load config from examples directory since we run from crate root
    let state_file = "examples/outputs/drain3.states";
    let config_path = "examples/drain3.toml";
    let config = if Path::new(config_path).exists() {
        TemplateMinerConfig::load(config_path).unwrap_or_default()
    } else {
        eprintln!("Config file not found at {}, using defaults", config_path);
        TemplateMinerConfig::default()
    };

    if !Path::new(state_file).exists() {
        println!("state file does not exist");
        std::process::exit(1);
    }

    let log_file_name = sample_logs::get_sample_logs().unwrap_or_else(|e| {
        println!("failed to get sample logs {}", e);
        std::process::exit(1);
    });

    let persistence = FilePersistence::new(state_file.to_string());
    let mut miner = TemplateMiner::new(&config, Some(Box::new(persistence)));

    let file = File::open(&log_file_name)?;
    let reader = BufReader::new(file);

    let start = Instant::now();
    let mut batch_start = start;
    let mut line_count = 0;

    let output_path = "examples/outputs/drain3_parameters.csv";
    let mut output_file = File::create(output_path)?;
    writeln!(output_file, "template_id,template,parameters")?;

    println!("Matching {}...", &log_file_name);

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let content = if let Some(idx) = line.find(": ") {
            &line[idx + 2..]
        } else {
            line
        };

        let log_cluster = match miner.match_cluster(content, SearchStrategy::Fallback) {
            Some(cluster) => cluster,
            None => {
                println!("failed to match line {}: {}", line_count + 1, &content);
                continue;
            }
        };

        let parameters = match miner.extract_parameters(&log_cluster.get_template(), content, true)
        {
            Some(params) => params,
            None => {
                println!(
                    "failed to extract parameters, template {}: {}\n line {}: {}",
                    log_cluster.cluster_id,
                    &log_cluster.get_template(),
                    line_count + 1,
                    &content
                );
                continue;
            }
        };

        let mut parameters_str = String::new();
        let mut comma = "";
        parameters.iter().for_each(|f| {
            parameters_str += &format!(
                "{}{}{} = {}{}",
                config.mask_prefix, &f.mask_name, config.mask_suffix, &f.value, comma
            );
            comma = ","
        });

        writeln!(
            output_file,
            "{},\"{}\",{}",
            log_cluster.cluster_id,
            log_cluster.get_template().replace("\"", "\"\""),
            &parameters_str,
        )?;

        line_count += 1;
        if line_count % 10000 == 0 {
            break;
            let now = Instant::now();
            let batch_duration = now.duration_since(batch_start);
            let batch_lines_sec = 10000.0 / batch_duration.as_secs_f64();
            println!(
                "Matching line: {}, rate {:.1} lines/sec, {} clusters.",
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
        "--- Done matching file in {:.2?} sec. Total of {} lines, rate {:.1} lines/sec, {} clusters",
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
