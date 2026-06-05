fn main() {
    println!("cargo::rustc-link-lib=dvdread");

    let bindings = bindgen::Builder::default()
        .header_contents(
            "wrapper.h",
            r#"
            #include <dvdread/dvd_reader.h>
            #include <dvdread/ifo_read.h>
            #include <dvdread/ifo_types.h>"#,
        )
        .allowlist_function("DVDOpen|DVDClose|ifoOpen|ifoClose")
        .allowlist_type("ifo_handle_t|tt_srpt_t|pgc_t|pgcit_t")
        .generate()
        .expect("Unable to generate bindings");

    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    dbg!("Bindings generated");
    bindings
        .write_to_file(out.join("bindings.rs"))
        .expect("Unable to write bindings");
}
