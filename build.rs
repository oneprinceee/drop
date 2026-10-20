use std::env;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use serde::Deserialize;
use walkdir::WalkDir;

#[allow(dead_code)]
#[derive(Deserialize)]
struct Config {
    app: AppConfig,
    install: InstallConfig,
    build: Option<BuildConfig>,
    hooks: Option<HooksConfig>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct AppConfig {
    name: String,
    version: String,
    publisher: String,
    main_exe: String,
    icon: Option<String>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct InstallConfig {
    source_dir: String,
    default_dir: String,
    create_desktop_shortcut: Option<bool>,
    create_start_menu_shortcut: Option<bool>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct BuildConfig {
    output_name: Option<String>,
    compression_level: Option<i32>,
}

#[derive(Deserialize)]
struct HooksConfig {
    post_install: Option<String>,
    pre_uninstall: Option<String>,
}

fn main() {
    println!("cargo:rerun-if-changed=CONFIG.toml");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let config_path = manifest_dir.join("CONFIG.toml");
    let mut config_file = File::open(&config_path).expect("failed to open CONFIG.toml");
    let mut config_str = String::new();
    config_file.read_to_string(&mut config_str).expect("failed to read CONFIG.toml");

    let config: Config = toml::from_str(&config_str).expect("failed to parse CONFIG.toml");

    let mut res = winres::WindowsResource::new();
    if let Some(icon_file) = &config.app.icon {
        let icon_path = manifest_dir.join(icon_file);
        if icon_path.exists() {
            println!("cargo:rerun-if-changed={}", icon_path.display());
            res.set_icon(icon_path.to_str().unwrap());
        }
    }
    res.compile().unwrap();

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    std::fs::write(out_dir.join("config.toml"), &config_str).unwrap();

    let mut post_install_bytes = Vec::new();
    if let Some(hooks) = &config.hooks {
        if let Some(post) = &hooks.post_install {
            let p = manifest_dir.join(post);
            println!("cargo:rerun-if-changed={}", p.display());
            if let Ok(mut f) = File::open(&p) {
                let _ = f.read_to_end(&mut post_install_bytes);
            }
        }
    }
    std::fs::write(out_dir.join("post_install.bin"), &post_install_bytes).unwrap();

    let mut pre_uninstall_bytes = Vec::new();
    if let Some(hooks) = &config.hooks {
        if let Some(pre) = &hooks.pre_uninstall {
            let p = manifest_dir.join(pre);
            println!("cargo:rerun-if-changed={}", p.display());
            if let Ok(mut f) = File::open(&p) {
                let _ = f.read_to_end(&mut pre_uninstall_bytes);
            }
        }
    }
    std::fs::write(out_dir.join("pre_uninstall.bin"), &pre_uninstall_bytes).unwrap();

    let src_dir = PathBuf::from(&config.install.source_dir);
    println!("cargo:rerun-if-changed={}", src_dir.display());

    let dest_path = out_dir.join("payload.tar.zst");
    let compression_level = config.build.as_ref().and_then(|b| b.compression_level).unwrap_or(19);

    let out_file = File::create(&dest_path).expect("failed to create payload file");
    let encoder = zstd::stream::Encoder::new(out_file, compression_level).unwrap();
    let mut tar_builder = tar::Builder::new(encoder);

    if src_dir.exists() {
        for entry in WalkDir::new(&src_dir) {
            let entry = entry.expect("failed to read dir entry");
            let path = entry.path();
            if path == src_dir {
                continue;
            }

            let rel_path = path.strip_prefix(&src_dir).unwrap();
            if path.is_file() {
                let mut f = File::open(path).expect("failed to open source file");
                tar_builder.append_file(rel_path, &mut f).expect("failed to append file to tar");
            } else if path.is_dir() {
                tar_builder.append_dir(rel_path, path).expect("failed to append dir to tar");
            }
        }
    }

    let encoder = tar_builder.into_inner().expect("failed to finish tar archive");
    encoder.finish().expect("failed to finish zstd compression");
}
