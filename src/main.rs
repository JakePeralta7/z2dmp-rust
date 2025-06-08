use std::env;
use std::path::Path;

use z2dmp::{logger, info};
use z2dmp::zdmp;

use z2dmp::result::{Result};

fn main()
-> Result<()> {
    // Log-level (default: info).
    let log_level = "info".to_string();
    
    let _log = logger::init(&log_level);

    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        panic!("Usage: {} <input_file> <output_file> [--silent]", args[0]);
    }

    let in_file = &args[1];
    let out_file = &args[2];

    // debug mode.
    let mut silent_mode = false; 
    if args.len() > 3 && args[3] == "--silent" {
        silent_mode = true;
        info!("Silent mode enabled");
    }
    
    info!("Input File:  {}", in_file);
    if !silent_mode {
        info!("Output File: {}", out_file);
    }
    
    // System info
    info!("CPU cores available: {}", num_cpus::get());
    info!("System memory optimization enabled");

    let zdmp_file = zdmp::ZdmpFile::new(Path::new(in_file), Path::new(out_file), silent_mode)?;

    let total_time = zdmp_file.finish_time - zdmp_file.start_time;

    info!("Expected file size:       0x{:x}", zdmp_file.file_size);
    info!("Actual decompressed size: 0x{:x}", zdmp_file.uncompressed_size);
    info!("Total blocks processed:   {}", zdmp_file.block_count);
    info!("Total decompression time: {:.2} secs", total_time.as_secs_f64());
    info!("Decompressed size:        {:.2} MB", (zdmp_file.uncompressed_size as f64) / (1024.0 * 1024.0));
    info!("Throughput:               {:.2} MB/s", 
        (zdmp_file.uncompressed_size as f64) / (1024.0 * 1024.0) / total_time.as_secs_f64());

    Ok(())
}
