import json
from typing import Optional, Any, Dict
import requests
import zlib
from time import sleep

class KVJob:
    def __init__(self, path_protein_pdb: str, path_ligand_pdb: Optional[str]=None):
        self.id: Optional[str] = None
        self.input: Optional[Dict[str, Any]] = {}
        self.output: Optional[Dict[str, Any]] = None 
        self._add_pdb(path_protein_pdb)
        if path_ligand_pdb != None:
            self._add_pdb(path_ligand_pdb, is_ligand=True)
        self._default_settings()

    @property
    def kv_pdb(self):
        if self.output == None:
            return None
        else:
            return self.output["output"]["pdb_kv"]

    @property
    def report(self):
        if self.output == None:
            return None
        else:
            return self.output["output"]["report"]

    @property
    def log(self):
        if self.output == None:
            return None
        else:
            return self.output["output"]["log"]

    def _add_pdb(self, pdb_fn: str, is_ligand: bool=False):
        with open(pdb_fn) as f:
            # pdb = f.readlines()
            pdb = f.read()
        if is_ligand:
            self.input["pdb_ligand"] = pdb
        else:
            self.input["pdb"] = pdb

    def _default_settings(self):
        self.input["settings"] = {}
        self.input["settings"]["modes"] = {
            "whole_protein_mode" : True,
            "box_mode" : False,
            "resolution_mode" : "Low",
            "surface_mode" : True,
            "kvp_mode" : False,
            "ligand_mode" : False,
        }
        self.input["settings"]["step_size"] = {"step_size": 0.0}
        self.input["settings"]["probes"] = {
            "probe_in" : 1.4,
            "probe_out" : 4.0,
        }
        self.input["settings"]["cutoffs"] = {
            "volume_cutoff" : 5.0,
            "ligand_cutoff" : 5.0,
            "removal_distance" : 0.0,
        }
        self.input["settings"]["visiblebox"] = {
            "p1" : {"x" : 0.00, "y" : 0.00, "z" : 0.00},
            "p2" : {"x" : 0.00, "y" : 0.00, "z" : 0.00},
            "p3" : {"x" : 0.00, "y" : 0.00, "z" : 0.00},
            "p4" : {"x" : 0.00, "y" : 0.00, "z" : 0.00},
        }
        self.input["settings"]["internalbox"] = {
            "p1" : {"x" : -4.00, "y" : -4.00, "z" : -4.00},
            "p2" : {"x" : 4.00, "y" : -4.00, "z" : -4.00},
            "p3" : {"x" : -4.00, "y" : 4.00, "z" : -4.00},
            "p4" : {"x" : -4.00, "y" : -4.00, "z" : 4.00},
        }

class KVClient:
    def __init__(self, server: str, port="80"):
        self.server = f"{server}:{port}"

    def run(self, kv_job: KVJob):
        if self._submit(kv_job):
            while kv_job.output == None:
                kv_job.output = self._get_results(kv_job)
                sleep(2)
            print("OK")

    def _submit(self, kv_job) -> bool:
        r = requests.post(self.server + '/create', json=kv_job.input)
        if r.ok:
            kv_job.id = r.json()['id']
            return True
        else:
            print("Debug:", r)
            print(r.text)
            return False

    def _get_results(self, kv_job) -> Optional[Dict[str, Any]]:
        r = requests.get(self.server + '/' + kv_job.id)
        if r.ok:
            results = r.json()
            if results['status'] == 'completed':
                return results
            else:
                print(results)
                return None
        else:
            print(r)
            return None
    

if __name__ == "__main__":
    # create and configure a KVClient with server url and port (default 80)
    # local server 
    kv = KVClient("http://localhost", "8081")
    # remote server
    # kv = KVClient("http://parkvfinder.cnpem.br", "8081")
    # create a job using a pdb file with default configuration (code to configure is not implemented)
    job = KVJob("examples/1FMO.pdb")
    # send job to server and wait until completion
    kv.run(job)
    # print job results
    print(json.dumps(job.output, indent=2))
