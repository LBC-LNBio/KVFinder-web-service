use kv;
use std::{thread, time};
use structopt::StructOpt;

#[derive(StructOpt)]
struct Cli {
    // KVFinder path
    kv_path: String,
    // path to save jobs
    job_path: String,
}

fn main() {
    println!("KVFinder Worker started");
    let args = Cli::from_args();
    let config = kv::worker::Config {
        kv_path: args.kv_path,
        job_path: args.job_path,
    };
    loop {
        let r = kv::worker::get_job();
        match r {
            Ok(j) => {
                let id = j.id;
                match kv::worker::process(j, &config) {
                    Err(e) => println!("Error processing: {}", e),
                    Ok(output) => match kv::worker::submit_result(id, output) {
                        Ok(id) => println!("Job processed successfully: {}", id),
                        Err(e) => println!("Error submitting result to queue: {}", e),
                    },
                }
            }
            //no job to process
            Err(_) => thread::sleep(time::Duration::from_secs(5)),
        }
    }
}
