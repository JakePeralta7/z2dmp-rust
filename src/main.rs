use std::env;
use std::path::Path;

use z2dmp::{logger, info};
use z2dmp::zdmp;

fn main() {
    // Log-level (default: info).
    let log_level = "info".to_string();
    
    let _log = logger::init(&log_level);

    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        panic!("Usage: {} <input_file> <output_file>", args[0]);
    }

    let in_file = &args[1];
    let out_file = &args[2];
    
    info!("Input File:  {}", in_file);
    info!("Output File: {}", out_file);

    let _zdmp_file = zdmp::ZdmpFile::new(Path::new(in_file), Path::new(out_file));
}
