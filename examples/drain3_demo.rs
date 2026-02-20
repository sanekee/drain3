use drain3::config::TemplateMinerConfig;

use drain3::FilePersistence;
use drain3::TemplateMiner;
use drain3::drain::LogCluster;
use drain3::drain::UpdateType;
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::time::Instant;

mod sample_logs;

struct SampleLine {
    line: i32,
    content: String,
    update_type: UpdateType,
}

impl std::fmt::Display for SampleLine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "line: {}, type: {}, {}",
            self.line, self.update_type, self.content,
        )
    }
}

type SampleLines = Vec<SampleLine>;

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

    let log_file_name = sample_logs::get_sample_logs().unwrap_or_else(|e| {
        println!("failed to get sample logs {}", e);
        std::process::exit(1);
    });

    // let persistence = FilePersistence::new(state_file.to_string());
    // let mut miner = TemplateMiner::new(config, Some(Box::new(persistence)));
    let mut miner = TemplateMiner::new(&config, None);

    let file = File::open(&log_file_name)?;
    let reader = BufReader::new(file);

    let start = Instant::now();
    let mut batch_start = start;
    let mut line_count = 0;

    let mut sample_lines: HashMap<usize, SampleLines> = HashMap::new();

    let output_path = "examples/outputs/drain3_output.csv";
    let mut output_file = File::create(output_path)?;
    writeln!(output_file, "template_id,size,template,samples")?;

    println!("Processing {}...", &log_file_name);

    for line in reader.lines() {
        let line_num = line_count + 1;
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

        let (cluster, update_type) = miner.add_log_message(content);

        let entry = sample_lines
            .entry(cluster.cluster_id)
            .or_insert_with(Vec::new);

        let exists = entry.iter().any(|sl| sl.update_type == update_type);

        if !exists {
            entry.push(SampleLine {
                line: line_num,
                content: content.to_string(),
                update_type,
            });
        }

        line_count += 1;
        if line_count % 10000 == 0 {
            break;
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

    println!("Prefix tree:");
    let mut stdout = io::stdout().lock();
    miner
        .drain
        .print_tree(&mut stdout, config.drain_max_clusters.unwrap_or_default())
        .unwrap();

    let mut clusters: Vec<&LogCluster> = miner.drain.id_to_cluster.values().collect();
    clusters.sort_by_key(|c| c.cluster_id);

    for cluster in clusters {
        let samples = sample_lines.get(&cluster.cluster_id);
        let sample_str = if let Some(lines) = samples {
            lines
                .iter()
                .map(|sl| format!("{}", sl))
                .collect::<Vec<_>>()
                .join("; ")
        } else {
            String::new()
        };
        writeln!(
            output_file,
            "{},{},\"{}\", \"{}\"",
            cluster.cluster_id,
            cluster.size,
            cluster.get_template().replace("\"", "\"\""),
            sample_str.replace("\"", "\"\"")
        )?;
    }

    Ok(())
}
