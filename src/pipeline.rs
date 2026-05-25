use std::{
    sync::mpsc::{self, Receiver},
    thread,
};

use crate::config::Config;
use anyhow::Result;

pub fn run_loop(cfg: &Config) -> Result<()> {
    let (tx, rx) = mpsc::channel::<EncodeJob>();

    // Spawn the background encode+upload worker
    let cfg_clone = cfg.clone();
    let worker = thread::spawn(move || {
        encode_upload_worker(rx, &cfg_clone);
    });

    loop {
        todo!("prompt for title");
        todo!("call makemkvcon");
        todo!("check if we should continue");
        break;
    }

    drop(tx);
    worker.join().expect("worker panicked");
    Ok(())
}

fn encode_upload_worker(rx: Receiver<EncodeJob>, cfg: &Config) {
    for job in rx {
        todo!("Encode and upload");
    }
}

pub struct EncodeJob {}
