/*!
image-verifier is a binary used as an image verification plugin with containerd.
*/
use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut digest = String::new();
    let mut image_name = String::new();
    
    // Parse CLI arguments
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-digest" => {
                if i + 1 < args.len() {
                    digest = args[i + 1].clone();
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "-name" => {
                if i + 1 < args.len() {
                    image_name = args[i + 1].clone();
                    i += 2;
                } else {
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }

    // Read JSON from stdin
    let mut buffer = String::new();
    if io::stdin().read_to_string(&mut buffer).is_err() {
        eprintln!("Failed to read from stdin");
        process::exit(1);
    }

    // Extract digest from JSON or use CLI digest
    let manifest_digest = if let Ok(json) = serde_json::from_str::<serde_json::Value>(&buffer) {
        json.get("digest")
            .and_then(|d| d.as_str())
            .unwrap_or(&digest)
            .to_string()
    } else {
        digest
    };

    // Check if image is from allowed registry
    if is_from_allowed_registry(&image_name) {
        println!("Image {} is from allowed registry", image_name);
        process::exit(0);
    }

    // Check allowlist
    let allowlist_path = env::var("ALLOWLIST_FILE")
        .unwrap_or_else(|_| "/etc/containerd/allowed-digests.txt".to_string());

    match is_allowed(&manifest_digest, &allowlist_path) {
        Ok(true) => {
            println!("Image manifest {} is allowed", manifest_digest);
            process::exit(0);
        }
        Ok(false) => {
            println!("Image manifest {} is not in allowlist", manifest_digest);
            process::exit(1);
        }
        Err(e) => {
            eprintln!("Error checking allowlist: {}", e);
            process::exit(1);
        }
    }
}

fn is_from_allowed_registry(image_name: &str) -> bool {
    let allowed_registries = [
        "registry.k8s.io/",
        "k8s.gcr.io/",
        "gcr.io/google-containers/"
    ];
    
    allowed_registries.iter().any(|registry| image_name.starts_with(registry))
}

fn is_allowed(digest: &str, allowlist_path: &str) -> Result<bool, Box<dyn std::error::Error>> {
    let file = File::open(allowlist_path)?;
    let reader = BufReader::new(file);
    
    for line in reader.lines() {
        if line?.trim() == digest {
            return Ok(true);
        }
    }
    Ok(false)
}
