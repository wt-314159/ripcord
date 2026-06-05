use anyhow::Result;
use std::collections::HashMap;
use std::ffi::CString;
use std::os::raw::c_int;

#[allow(warnings)]
mod bindings {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}
use bindings::*;

#[derive(Debug)]
pub struct TitleInfo {
    pub title_number: usize, // 1-indexed, in disc order
    pub vts: u32,            // which VTS_XX is this title in
    pub chapters: u32,       // number of PTTs
    pub duration_secs: f64,
    pub fps: u32,
}

pub fn read_titles(path: &str) -> Result<Vec<TitleInfo>> {
    let c_path = CString::new(path)?;

    unsafe {
        let dvd = DVDOpen(c_path.as_ptr());
        if dvd.is_null() {
            return Err(anyhow::anyhow!(format!("Failed to open DVD for {path}")));
        }

        // ifoOpen(dvd, 0) opens VIDEO_TS.IFO
        let vmg = ifoOpen(dvd, 0);
        if vmg.is_null() {
            return Err(anyhow::anyhow!(format!("ifoOpen failed for {path}")));
        }

        let tt_srpt = (*vmg).__bindgen_anon_1.__bindgen_anon_1.tt_srpt; //tt_srpt;
        let n_titles = (*tt_srpt).nr_of_srpts as usize;

        let mut vts_map = HashMap::new();

        let mut out = Vec::with_capacity(n_titles);
        for i in 0..n_titles {
            let title = (*tt_srpt).title.add(i);
            let vts_n = (*title).title_set_nr as c_int;
            let ttn = (*title).vts_ttn as usize;
            let n_ptts = (*title).nr_of_ptts as u32;

            // Open the VTS this title belongs to
            let vts = vts_map.entry(vts_n).or_insert_with(|| ifoOpen(dvd, vts_n));

            if vts.is_null() {
                continue; // skip broken titles
            }

            // PTT table: find the program chain for this title's first chapter
            let ptt_srpt = (*(*vts)).__bindgen_anon_1.__bindgen_anon_1.vts_ptt_srpt;
            let ptt_title = (*ptt_srpt).title.add(ttn - 1);
            let ptt = (*ptt_title).ptt; // pointer to first ptt entry
            let pgcn = (*ptt).pgcn as usize;

            // Look up that PGC in the VTS PGC table
            let pgcit = (*(*vts)).__bindgen_anon_1.__bindgen_anon_1.vts_pgcit;
            let pgci_srp = (*pgcit).pgci_srp.add(pgcn - 1);
            let pgc = (*pgci_srp).pgc;

            let duration_secs = dvd_time_to_seconds(&(*pgc).playback_time);
            let fps = dvd_time_to_fps(&(*pgc).playback_time);

            out.push(TitleInfo {
                title_number: i + 1,
                vts: vts_n as u32,
                chapters: n_ptts,
                duration_secs,
                fps,
            })
        }
        Ok(out)
    }
}

fn bcd_to_int(b: u8) -> u32 {
    ((b >> 4) as u32) * 10 + (b & 0x0f) as u32
}

/// Convert a dvd_time_t into total seconds
fn dvd_time_to_seconds(t: &dvd_time_t) -> f64 {
    let hours = bcd_to_int(t.hour);
    let minutes = bcd_to_int(t.minute);
    let seconds = bcd_to_int(t.second);

    // Frame rate from top two bits of frame_u
    let fps = match (t.frame_u & 0xc0) >> 6 {
        0b11 => 30.0,
        0b10 => 25.0,
        _ => 0.0, // unknown
    };
    let frames = bcd_to_int(t.frame_u & 0x3f);

    let mut total = (hours as f64) * 3600.0 + (minutes as f64) * 60.0 + seconds as f64;
    if fps > 0.0 {
        total += frames as f64 / fps;
    }
    total
}

/// Convert a dvd_time_t to frame rate
fn dvd_time_to_fps(t: &dvd_time_t) -> u32 {
    match (t.frame_u & 0xc0) >> 6 {
        0b11 => 30,
        0b10 => 25,
        _ => 0, // unknown
    }
}
