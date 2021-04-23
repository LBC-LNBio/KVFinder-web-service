mod kv {
    pub mod worker;
    pub mod webserver;

    extern crate base64;
    extern crate zstd;
    use serde::{Deserialize, Serialize};
    use std::io;
    use std::error::Error;

    #[derive(Serialize, Deserialize, Debug)]
    struct KVParameters {
        title: String,
        files_path: KVFilesPath,
        settings: KVSettings,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct KVFilesPath {
        dictionary: String,
        pdb: String,
        output: String,
        base_name: String,
        ligand: String,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(deny_unknown_fields)]
    struct KVSettings {
        modes: KVSModes,
        step_size: KVSStepSize,
        probes: KVSProbes,
        cutoffs: KVSCutoffs,
        visiblebox: KVSVisiblebox,
        internalbox: KVSInternalbox,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(deny_unknown_fields)]
    struct KVSModes {
        whole_protein_mode: bool,
        box_mode: bool,
        resolution_mode: KVSResolution,
        surface_mode: bool,
        kvp_mode: bool,
        ligand_mode: bool,
    }

    #[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
    #[serde(deny_unknown_fields)]
    enum KVSResolution {
        Low,
        Medium,
        High,
        Off,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(deny_unknown_fields)]
    struct KVSStepSize {
        step_size: f64,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(deny_unknown_fields)]
    struct KVSProbes {
        probe_in: f64,
        probe_out: f64,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(deny_unknown_fields)]
    struct KVSCutoffs {
        volume_cutoff: f64,
        ligand_cutoff: f64,
        removal_distance: f64,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(deny_unknown_fields)]
    struct KVSVisiblebox {
        p1: KVSBoxPoint,
        p2: KVSBoxPoint,
        p3: KVSBoxPoint,
        p4: KVSBoxPoint,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(deny_unknown_fields)]
    struct KVSInternalbox {
        p1: KVSBoxPoint,
        p2: KVSBoxPoint,
        p3: KVSBoxPoint,
        p4: KVSBoxPoint,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(deny_unknown_fields)]
    struct KVSBoxPoint {
        x: f64,
        y: f64,
        z: f64,
    }

    struct PdbBoundaries {
        x_min: f64,
        x_max: f64,
        y_min: f64,
        y_max: f64,
        z_min: f64,
        z_max: f64,
    }

    impl PdbBoundaries {
        // check if kvbox is inside pdb boundaries
        fn contains(&self, kvbox: &KVSInternalbox) -> bool {
            if self.x_min > kvbox.p1.x
                || self.x_min > kvbox.p2.x
                || self.x_min > kvbox.p3.x
                || self.x_min > kvbox.p4.x
            {
                return false;
            }
            if self.x_max < kvbox.p1.x
                || self.x_max < kvbox.p2.x
                || self.x_max < kvbox.p3.x
                || self.x_max < kvbox.p4.x
            {
                return false;
            }
            if self.y_min > kvbox.p1.y
                || self.y_min > kvbox.p2.y
                || self.y_min > kvbox.p3.y
                || self.y_min > kvbox.p4.y
            {
                return false;
            }
            if self.y_max < kvbox.p1.y
                || self.y_max < kvbox.p2.y
                || self.y_max < kvbox.p3.y
                || self.y_max < kvbox.p4.y
            {
                return false;
            }
            if self.z_min > kvbox.p1.z
                || self.z_min > kvbox.p2.z
                || self.z_min > kvbox.p3.z
                || self.z_min > kvbox.p4.z
            {
                return false;
            }
            if self.z_max < kvbox.p1.z
                || self.z_max < kvbox.p2.z
                || self.z_max < kvbox.p3.z
                || self.z_max < kvbox.p4.z
            {
                return false;
            }
            true
        }
    }

    // Compression and decompression are applied to text data (pdb file content,
    // parkvfinder results) respectively when they are sent and get from queue to
    // reduce ocypod (redis) memory usage.
    // zstd 1 shows more compression than 3 (~4x for pdb files) in addition
    // to better performance
    // base64 representation increases binary size in 1/3 (string size = 4/3 * binary size)
    // the combination results in a compression ratio of ~3x
    fn compress(s: &String) -> Result<String, io::Error> {
        let v = zstd::block::compress(s.as_bytes(), 1)?;
        Ok(base64::encode(&v))
    }

    fn decompress(b64: &String) -> Result<String, Box<dyn Error>> {
        let s = base64::decode(&b64)?;
        Ok(String::from_utf8(zstd::stream::decode_all(s.as_slice())?)?)
    }


    #[derive(Serialize, Deserialize)]
    struct Data {
        tags: [String; 1],
        input: Input,
    }

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct Input {
        settings: KVSettings,
        pdb: String,
        pdb_ligand: Option<String>,
    }

    impl Input {
        /// Check if parameters received from a client (users) are ok.
        /// Some parameters have constraints in this web service to prevent heavy
        /// jobs that could block or slow down the server.
        fn check(&self) -> Result<(), &str> {
            // Compare Whole protein and Box modes
            if self.settings.modes.whole_protein_mode == self.settings.modes.box_mode {
                return Err(
                    "Invalid parameters file! Whole protein and box modes cannot be equal!",
                );
            }
            // Compare resolution mode
            if self.settings.modes.resolution_mode != KVSResolution::Low {
                return Err("Invalid parameters file! Resolution mode is restricted to Low option on this web service!");
            }
            // Probe In
            if self.settings.probes.probe_in < 0.0 || self.settings.probes.probe_in > 5.0 {
                return Err("Invalid parameters file! Probe In must be between 0 and 5!");
            }
            // Probe Out
            if self.settings.probes.probe_out < 0.0 || self.settings.probes.probe_out > 50.0 {
                return Err("Invalid parameters file! Probe Out must be between 0 and 50!");
            }
            // Compare probes
            if self.settings.probes.probe_out < self.settings.probes.probe_in {
                return Err("Invalid parameters file! Probe Out must be greater than Probe In!");
            }
            // Removal distance
            if self.settings.cutoffs.removal_distance < 0.0
                || self.settings.cutoffs.removal_distance > 10.0
            {
                return Err("Invalid parameters file! Removal distance must be between 0 and 10!");
            }
            // Volume Cutoff
            if self.settings.cutoffs.volume_cutoff < 0.0 {
                return Err("Invalid parameters file! Volume cutoff must be greater than 0!");
            }
            // Cavity representation
            if self.settings.modes.kvp_mode {
                return Err("Invalid parameters file! Cavity Representation (kvp_mode) must be false on this webservice!");
            }
            // Ligand mode and pdb
            if self.settings.modes.ligand_mode && self.pdb_ligand == None {
                return Err("Invalid parameters file! A ligand must be provided when Ligand mode is set to true!");
            } else if !self.settings.modes.ligand_mode && self.pdb_ligand != None {
                return Err("Invalid parameters file! The Ligand mode must be set to true when providing a ligand!");
            }
            // Ligand Cutoff
            if self.settings.cutoffs.ligand_cutoff <= 0.0 {
                return Err("Invalid parameters file! Ligand cutoff must be greater than 0!");
            }

            // Box inside pdb boundaries
            if self.settings.modes.box_mode {
                if let Ok(pdb_boundaries) = self.get_pdb_boundaries() {
                    if !pdb_boundaries.contains(&self.settings.internalbox) {
                        return Err("Invalid parameters file! Inconsistent box coordinates!");
                    }
                } else {
                    return Err("parsing error");
                }
            }
            Ok(())
        }

        /// Get boundaries of a PDB file.
        /// Boundaries are defined as minimum/maximum values for each cartesian axis with
        /// subtraction/addition of probe value plus 20 angstrons.
        fn get_pdb_boundaries(&self) -> Result<PdbBoundaries, &str> {
            let coords: Option<PdbBoundaries> = self
                .pdb
                .lines()
                .filter(|s| s.starts_with("ATOM"))
                .map(|s| {
                    let (x, y, z) = (
                        s.get(30..38)?.trim().parse::<f64>().ok()?,
                        s.get(38..46)?.trim().parse::<f64>().ok()?,
                        s.get(46..54)?.trim().parse::<f64>().ok()?,
                    );
                    Ok((x, y, z))
                })
                .fold(None as Option<PdbBoundaries>, |state, p| {
                    let p = p?;
                    match state {
                        None => Some(PdbBoundaries {
                            x_min: p.0,
                            x_max: p.0,
                            y_min: p.1,
                            y_max: p.1,
                            z_min: p.2,
                            z_max: p.2,
                        }),
                        Some(s) => Some(PdbBoundaries {
                            x_min: s.x_min.min(p.0),
                            x_max: s.x_max.max(p.0),
                            y_min: s.y_min.min(p.1),
                            y_max: s.y_max.max(p.1),
                            z_min: s.z_min.min(p.2),
                            z_max: s.z_max.max(p.2),
                        }),
                    }
                });

            match coords {
                Some(c) => Ok(PdbBoundaries {
                    // we define pdb boundaries adding probe out and also 20 angstrons to each
                    // direction
                    x_min: c.x_min - (self.settings.probes.probe_out + 20.0),
                    x_max: c.x_max + (self.settings.probes.probe_out + 20.0),
                    y_min: c.y_min - (self.settings.probes.probe_out + 20.0),
                    y_max: c.y_max + (self.settings.probes.probe_out + 20.0),
                    z_min: c.z_min - (self.settings.probes.probe_out + 20.0),
                    z_max: c.z_max + (self.settings.probes.probe_out + 20.0),
                }),
                None => Err("parsing error"),
            }
        }
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Output {
        pdb_kv: String,
        report: String,
        log: String,
    }
}

pub use crate::kv::webserver;
pub use crate::kv::worker;
