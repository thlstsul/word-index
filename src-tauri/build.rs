use pandoc::setup_pandoc;

fn main() {
    #[cfg(feature = "pandoc")]
    setup_pandoc().expect("Could not load the pandoc assets");

    tauri_build::build();
}

#[cfg(feature = "pandoc")]
mod pandoc {
    use std::env;
    use std::fs::{create_dir_all, File, OpenOptions};
    use std::io::{BufReader, BufWriter, Cursor, Read, Write};
    use std::path::PathBuf;
    use std::time::Duration;

    use anyhow::Context;
    use cargo_toml::Manifest;
    use sha1::{Digest, Sha1};

    pub fn setup_pandoc() -> anyhow::Result<()> {
        let cargo_manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
        let cargo_toml = cargo_manifest_dir.join("Cargo.toml");
        let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
        let bin_dir = PathBuf::from("bin");

        let pandoc_dir = out_dir.join("pandoc");
        let sha1_path = out_dir.join(".pandoc.sha1");

        let manifest = Manifest::from_path(cargo_toml).unwrap();

        let meta = &manifest
            .package
            .as_ref()
            .context("package not specified in Cargo.toml")?
            .metadata
            .as_ref()
            .context("no metadata specified in Cargo.toml")?["pandoc"];

        // Check if there already is a dashboard built, and if it is up to date.
        if sha1_path.exists() && pandoc_dir.exists() {
            let mut sha1_file = File::open(&sha1_path)?;
            let mut sha1 = String::new();
            sha1_file.read_to_string(&mut sha1)?;
            if sha1 == meta["sha1"].as_str().unwrap() {
                // Nothing to do.
                return Ok(());
            }
        }

        let url = meta["assets-url"].as_str().unwrap();

        println!("下载pandoc中……");
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(360000))
            .danger_accept_invalid_certs(true)
            .build()?;

        let pandoc_assets_bytes = client.get(url).send()?.bytes()?;

        let mut hasher = Sha1::new();
        hasher.update(&pandoc_assets_bytes);
        let sha1 = hex::encode(hasher.finalize());

        assert_eq!(
            meta["sha1"].as_str().unwrap(),
            sha1,
            "Downloaded pandoc shasum differs from the one specified in the Cargo.toml"
        );

        create_dir_all(&pandoc_dir)?;
        let cursor = Cursor::new(&pandoc_assets_bytes);
        let mut zip = zip::read::ZipArchive::new(cursor)?;
        zip.extract(&pandoc_dir)?;

        // copy to bin
        let pandoc_bin_dir = {
            pandoc_dir
                .as_path()
                .read_dir()?
                .next()
                .unwrap()?
                .path()
        };
        
        if cfg!(windows) {
            let pandoc_bin = File::open(pandoc_bin_dir.join("pandoc.exe"))?;
            let bin = File::create(bin_dir.join("pandoc-x86_64-pc-windows-msvc.exe"))?;
            println!("{:?} -> {:?}", pandoc_bin, bin);
            std::io::copy(&mut BufReader::new(pandoc_bin), &mut BufWriter::new(bin))?;
        }

        // Write the sha1 for the dashboard back to file.
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(sha1_path)?;

        file.write_all(sha1.as_bytes())?;
        file.flush()?;

        Ok(())
    }
}
