use dive_api::{CheckRequest, Store, check_file};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = std::path::PathBuf::from(std::env::args().nth(1).ok_or("source required")?);
    for compat_tlc in [false, true] {
        for store in [Store::Fingerprint, Store::Exact] {
            for workers in [1, 4, 8] {
                for chaos in [None, Some(19)] {
                    let result = check_file(&CheckRequest {
                        file: file.clone(), compat_tlc, store, workers: Some(workers),
                        chaos, seed: Some(7), fp: Some(0), ..CheckRequest::default()
                    })?;
                    let json = result.to_json();
                    assert_eq!(result.exit_code(), 3);
                    assert_eq!(json["verdict"], "unsupported");
                    assert_eq!(json["options"]["workers"]["requested"], workers);
                    assert_eq!(json["options"]["workers"]["used"], 0);
                    assert!(json["statistics"].is_null() && json["counterexample"].is_null());
                    assert_eq!(json["output"].as_array().map(Vec::len), Some(0));
                    assert_eq!(json["warnings"].as_array().map(Vec::len), Some(0));
                }
            }
        }
    }
    println!("24 public Rust configured-native VIEW admission controls passed");
    Ok(())
}
