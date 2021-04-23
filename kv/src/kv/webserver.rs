use super::{Data, Input, Output};
use actix_web::{web, HttpResponse, Responder};
use fasthash::city;
use reqwest;
use serde::{Deserialize, Serialize};
use serde_json;
use serde_json::json;

#[derive(Serialize, Deserialize)]
struct Job {
    #[serde(default)]
    id: String, // this id is the same as tag_id (NOT queue_id)
    status: String,
    output: Option<Output>,
    created_at: String,
    started_at: Option<String>,
    ended_at: Option<String>,
    expires_after: String,
}

#[derive(Serialize, Deserialize)]
struct QueueConfig<'a> {
    timeout: &'a str,
    expires_after: &'a str,
    retries: i32,
}

// GET /
pub async fn hello() -> impl Responder {
    "KVFinder Web Service"
}

pub fn create_ocypod_queue(
    queue_name: &str,
    timeout: &str,
    expires_after: &str,
    retries: i32,
) {
    let client = reqwest::Client::new();
    let queue_url = format!("http://ocypod:8023/queue/{}", queue_name);
    let queue_config = QueueConfig {
        timeout,
        expires_after,
        retries,
    };
    let _response = client.put(&queue_url).json(&queue_config).send();
    // match _response {
    //     Ok(_) => HttpResponse::Ok().json(json!({"id":data.tags[0]})),
    //     Err(e) => HttpResponse::InternalServerError().body(format!("{:?}", e)),
    // }
}


/// Get job queue id for a job "tag id". The tag id is created applying a hash
/// function to received data (input data). It is the id sent to users. The queue
/// id is for internal use only and increase sequentially.
/// If tag id is not found returns Ok(None).
fn get_queue_id(tag_id: &String) -> Result<Option<u32>, reqwest::Error> {
    let url = format!("http://ocypod:8023/tag/{}", tag_id);

    // ids because in theory could be more than one with the same tag, BUT if this happen there is an error
    // if tag_id (hash64) not found in queue Ok(None)
    // if request fail return Err (possible problem in queue server)
    let mut ids: Vec<u32> = reqwest::get(url.as_str())?.json()?;
    // pop returns last id (should have only one or zero) or None
    Ok(ids.pop())
}

/// Use tag id job to get job data from queue.
/// If tag id not found returns Ok(None).
fn get_job(tag_id: String) -> Result<Option<Job>, reqwest::Error> {
    let queue_id = get_queue_id(&tag_id);
    let job = |queue_id| {
        let url = format!("http://ocypod:8023/job/{}?fields=status,output,created_at,started_at,ended_at,expires_after", queue_id);
        let mut j: Job = reqwest::get(url.as_str())?.json()?;
        j.id = tag_id;
        if let Some(output) =  &mut j.output {
            output.pdb_kv = super::decompress(&output.pdb_kv).expect("decompression error");
            output.report = super::decompress(&output.report).expect("decompression error");
            output.log = super::decompress(&output.log).expect("decompression error");
        }
        // j.output.pdb_kv = super::decompress(&j.output.pdb_kv).expect("decompression error");

        Ok(Some(j))
    };

    match queue_id {
        Err(e) => Err(e),
        // if queue_id is None (tag_id not found)
        Ok(None) => return Ok(None),
        // return job data in json
        Ok(Some(queue_id)) => return job(queue_id),
    }
}

/// GET /:id
/// This :id requested by by users through HTTP is the tag id.
/// If the :id is found returns an HTTP response with output data which includes
/// processing status: "queued", "running", "completed"...
/// If :id is not found returns NOT FOUND (Code 404)
pub async fn ask(id: web::Path<String>) -> impl Responder {
    let tag_id = id.into_inner();
    let job = get_job(tag_id);
    match job {
        Err(e) => HttpResponse::InternalServerError().body(format!("{:?}", e)),
        Ok(None) => HttpResponse::NotFound().finish(),
        Ok(Some(j)) => HttpResponse::Ok().json(j),
    }
}

/// POST /create
/// Receives input data (json sent by users), creates a job, sends it to queue and
/// responds the user (http response) with the job id.
/// Also, before create a job, it checks if a job with the same parameters (hash -> tag id)
/// are not yet into queue. If it is, it responds with job data.
pub async fn create(job_input: web::Json<Input>) -> impl Responder {
    // json input values to inp
    let input = job_input.into_inner();
    if let Err(e) = &input.check() {
        return HttpResponse::BadRequest().body(format!("{:?}", e));
    }
    // compress pdb data to reduce queue memory usage.
    let compressed_input = Input {
        pdb: super::compress(&input.pdb).expect("compression error"),
        pdb_ligand: match input.pdb_ligand { 
            Some(lig) => Some(super::compress(&lig).expect("compression error")),
            None => None,
        },
        ..input
    };
    let data = Data {
        // create a tag using function hash64 applied to input (unique value per input)
        tags: [city::hash64(serde_json::to_string(&compressed_input).unwrap()).to_string()],
        input: compressed_input,
    };
    // closure to sends data to queue
    let create_job = || {
        let client = reqwest::Client::new();
        let response = client
            .post("http://ocypod:8023/queue/kvfinder/job")
            .json(&data)
            .send();
        match response {
            Ok(_) => HttpResponse::Ok().json(json!({"id":data.tags[0]})),
            Err(e) => HttpResponse::InternalServerError().body(format!("{:?}", e)),
        }
    };
    let job = get_job(data.tags[0].clone());
    match job {
        // if err, problem in queue server
        Err(e) => HttpResponse::InternalServerError().body(format!("{:?}", e)),
        // if job with this tag is in queue, return job
        Ok(Some(j)) => HttpResponse::Ok().json(j),
        // if job with this tag is not found on queue, create job
        Ok(None) => create_job(), //format!("{} created", tag_id),
    }
}
