//! Generate large content files for testing.
//!
//! Run: cargo run -q --example large_content -p keva_core

use keva_core::core::KevaCore;
use keva_core::types::{Config, Key, SavedConfig};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

const TEST_SIZES_MB: &[usize] = &[1, 10, 50, 100];

fn main() {
    let config = Config {
        base_path: get_data_path(),
        saved: SavedConfig {
            trash_ttl: Duration::from_secs(30 * 24 * 60 * 60),
            purge_ttl: Duration::from_secs(7 * 24 * 60 * 60),
        },
    };

    let mut keva = KevaCore::open(config).expect("Failed to open database");
    let now = SystemTime::now();

    println!("Generating large content test files...\n");

    for &size_mb in TEST_SIZES_MB {
        let key_name = format!("large-test-{}mb", size_mb);
        let key = Key::try_from(key_name.as_str()).expect("Invalid key name");

        print!("  {} ... ", key_name);

        if keva.get(&key).ok().flatten().is_none() {
            keva.create(&key, now).expect("Failed to create key");
        }

        let content = generate_markdown_content(size_mb);
        let line_count = content.lines().count();
        let content_path = keva.content_path(&key);
        std::fs::write(&content_path, &content).expect("Failed to write content");
        keva.touch(&key, now).expect("Failed to touch key");

        println!("{} bytes, {} lines", content.len(), line_count);
    }

    println!("\nDone.");
}

fn generate_markdown_content(size_mb: usize) -> String {
    let target_bytes = size_mb * 1024 * 1024;
    let mut content = String::with_capacity(target_bytes + 1024);

    content.push_str("# Large File Test\n\n");
    content.push_str(&format!("Target size: {}MB\n\n", size_mb));
    content.push_str("---\n\n");

    let mut line_num = 1;
    let sample_lines = [
        "## Section {n}\n",
        "\n",
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore.\n",
        "\n",
        "```rust\n",
        "fn example_{n}() {{\n",
        "    let value = {n};\n",
        "    println!(\"Line {n}: {{}}\", value);\n",
        "}}\n",
        "```\n",
        "\n",
        "- Item {n}.1\n",
        "- Item {n}.2\n",
        "- Item {n}.3\n",
        "\n",
        "> Quote block {n}\n",
        "\n",
        "| Column A | Column B | Column C |\n",
        "|----------|----------|----------|\n",
        "| Cell {n} | Data     | Value    |\n",
        "\n",
    ];

    while content.len() < target_bytes {
        for line in &sample_lines {
            content.push_str(&line.replace("{n}", &line_num.to_string()));
        }
        line_num += 1;
    }

    content.push_str("\n## End of File\n");

    content
}

fn get_data_path() -> PathBuf {
    std::env::var("KEVA_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::var("LOCALAPPDATA")
                .map(PathBuf::from)
                .expect("LOCALAPPDATA not set")
                .join("keva")
        })
}
