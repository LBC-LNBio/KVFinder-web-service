use super::{Input, Output};
use reqwest;
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::{create_dir, File};
use std::io;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use toml;

#[derive(Serialize, Deserialize, Debug)]
pub struct JobInput {
    pub id: u32,
    input: Input,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct JobOutput {
    status: String,
    output: Output,
}

pub struct Config {
    pub kv_path: String,
    pub job_path: String,
}

impl JobInput {
    /// Save config file
    fn save(&self, config: &Config) -> Result<(), io::Error> {
        self.input.save(self.id, &config)?;
        Ok(())
    }

    /// Call parkvfinder command and get results.
    fn run(&self, config: &Config) -> Result<Output, io::Error> {
        let kvfinder = Command::new(format!("{}/parKVFinder", config.kv_path))
            .current_dir(format!("{}/{}", config.job_path, self.id))
            .arg("-p")
            .arg("params.toml")
            .status()
            .expect("failed to execute KVFinder process");
        println!("process exited with: {}", kvfinder);
        if kvfinder.success() {
            // read results from files and compress
            let kv_pdb_output = super::compress(&fs::read_to_string(format!(
                "{}/{}/KV_Files/KVFinderWeb/KVFinderWeb.KVFinder.output.pdb",
                config.job_path, self.id
            ))?).expect("compression error");
            let kv_report = super::compress(&fs::read_to_string(format!(
                "{}/{}/KV_Files/KVFinderWeb/KVFinderWeb.KVFinder.results.toml",
                config.job_path, self.id
            ))?).expect("compression error");
            let kv_log = super::compress(&fs::read_to_string(format!(
                "{}/{}/KV_Files/KVFinder.log",
                config.job_path, self.id
            ))?).expect("compression error");
            let output = Output {
                pdb_kv: kv_pdb_output,
                report: kv_report,
                log: kv_log,
            };
            println!("KVFinder OK");
            return Ok(output);
        } else {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "oh no! check if variable KVFinder_PATH was set",
            ));
        }
    }
}

impl Input {
    // save files to parkvfinder process them.
    fn save(&self, id: u32, config: &Config) -> Result<(), io::Error> {
        let dir = format!("{}/{}", config.job_path, id);
        match create_dir(&dir) {
            Err(err) => Err(err),
            Ok(_) => {
                self.save_parameters(&dir, &config)?;
                self.save_pdb(&dir)?;
                if let Some(_) = self.pdb_ligand {
                    self.save_pdb_ligand(&dir)?;
                }
                Ok(())
            }
        }
    }

    fn save_parameters(&self, dir: &str, config: &Config) -> Result<(), io::Error> {
        let params = super::KVParameters {
            title: String::from("KVFinder-worker parameters"),
            files_path: super::KVFilesPath {
                dictionary: String::from(format!("{}/dictionary", config.kv_path)),
                pdb: String::from("./protein.pdb"),
                ligand: String::from("./ligand.pdb"),
                output: String::from("./"),
                base_name: String::from("KVFinderWeb"),
            },
            settings: self.settings.clone(),
        };
        let toml_parameters = toml::to_string(&params);
        let filename = format!("{}/params.toml", dir);
        let path = Path::new(&filename);
        let mut file = File::create(path)?;
        if let Ok(p) = toml_parameters {
            writeln!(file, "{}", p)?;
        }
        Ok(())
    }

    fn save_pdb(&self, dir: &str) -> Result<(), io::Error> {
        let filename = format!("{}/protein.pdb", dir);
        let path = Path::new(&filename);
        let mut file = File::create(path)?;
        writeln!(file, "{}", super::decompress(&self.pdb).expect("decompression error"))?;
        Ok(())
    }

    fn save_pdb_ligand(&self, dir: &str) -> Result<(), io::Error> {
        let filename = format!("{}/ligand.pdb", dir);
        let path = Path::new(&filename);
        let mut file = File::create(&path)?;
        if let Some(pdb_ligand) = &self.pdb_ligand {
            writeln!(file, "{}", super::decompress(&pdb_ligand).expect("decompression error"))?;
        }
        Ok(())
    }
}

/// Get next job from queue. Returns Error if there is not a job to process.
pub fn get_job() -> Result<JobInput, reqwest::Error> {
    // the queue name "kvfinder" is hardcoded at bin/kv_server.rs
    let j: JobInput = reqwest::get("http://ocypod:8023/queue/kvfinder/job")?.json()?;
    Ok(j)
}

pub fn process(job: JobInput, config: &Config) -> Result<Output, io::Error> {
    job.save(&config)?;
    let output = job.run(&config);
    output
}

pub fn submit_result(id: u32, output: Output) -> Result<u32, reqwest::Error> {
    let client = reqwest::Client::new();
    let url = format!("http://ocypod:8023/job/{}", id);
    let  data = JobOutput {
        status: String::from("completed"),
        output,
    };
    // update job at queue
    let _result = client.patch(url.as_str()).json(&data).send()?;
    Ok(id)
}
