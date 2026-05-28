//! Dev bundle builder for Reactor fixtures.
//!
//! Compiles WAT to WASM, zips function directories, computes SHA-256 hashes,
//! rewrites manifest.json, and packs everything into a deploy bundle.

use anyhow::{Context, Result};
use reactor_deploy_bundle::BundleManifest;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    let (src_dir, out_path) = parse_args(&args)?;

    println!("Building bundle from: {}", src_dir.display());

    // Step 1: Compile WAT to WASM for each function
    compile_wat_to_wasm(&src_dir)?;

    // Step 2: Zip function directories
    zip_function_dirs(&src_dir)?;

    // Step 3: Compute hashes and rewrite manifest
    rewrite_manifest(&src_dir)?;

    // Step 4: Pack the bundle
    let bundle_data = reactor_deploy_bundle::pack(&src_dir)
        .context("Failed to pack bundle")?;

    // Step 5: Write output
    fs::write(&out_path, &bundle_data)
        .with_context(|| format!("Failed to write bundle to {}", out_path.display()))?;

    println!("Bundle written to: {} ({} bytes)", out_path.display(), bundle_data.len());
    Ok(())
}

fn parse_args(args: &[String]) -> Result<(PathBuf, PathBuf)> {
    let mut src_dir = None;
    let mut out_path = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--src" => {
                i += 1;
                src_dir = Some(PathBuf::from(&args[i]));
            }
            "--out" => {
                i += 1;
                out_path = Some(PathBuf::from(&args[i]));
            }
            _ => anyhow::bail!("Unknown argument: {}", args[i]),
        }
        i += 1;
    }

    let src_dir = src_dir.context("--src is required")?;
    let out_path = out_path.context("--out is required")?;

    Ok((src_dir, out_path))
}

fn compile_wat_to_wasm(src_dir: &Path) -> Result<()> {
    let functions_dir = src_dir.join("functions");
    if !functions_dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(&functions_dir)? {
        let entry = entry?;
        let fn_dir = entry.path();
        if !fn_dir.is_dir() {
            continue;
        }

        let wat_path = fn_dir.join("main.wat");
        let code_dir = fn_dir.join("code");
        let wasm_path = code_dir.join("main.wasm");

        if wat_path.exists() && !wasm_path.exists() {
            println!("Compiling: {} -> {}", wat_path.display(), wasm_path.display());

            // Ensure code/ directory exists
            fs::create_dir_all(&code_dir)?;

            // Compile WAT to WASM
            let wat_source = fs::read_to_string(&wat_path)
                .with_context(|| format!("Failed to read {}", wat_path.display()))?;
            let wasm_bytes = wat::parse_str(&wat_source)
                .with_context(|| format!("Failed to compile {}", wat_path.display()))?;

            fs::write(&wasm_path, wasm_bytes)
                .with_context(|| format!("Failed to write {}", wasm_path.display()))?;
        }
    }

    Ok(())
}

fn zip_function_dirs(src_dir: &Path) -> Result<()> {
    let functions_dir = src_dir.join("functions");
    if !functions_dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(&functions_dir)? {
        let entry = entry?;
        let fn_dir = entry.path();
        if !fn_dir.is_dir() {
            continue;
        }

        let fn_name = fn_dir.file_name().unwrap().to_str().unwrap();
        let zip_path = functions_dir.join(format!("{}.zip", fn_name));

        println!("Zipping: {} -> {}", fn_dir.display(), zip_path.display());

        let file = fs::File::create(&zip_path)?;
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        // Collect and sort entries for deterministic output
        let mut entries: Vec<_> = WalkDir::new(&fn_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| {
                // Skip .wat files, they're source only
                e.path().extension().map(|s| s != "wat").unwrap_or(true)
            })
            .collect();
        entries.sort_by(|a, b| a.path().cmp(b.path()));

        for entry in entries {
            let path = entry.path();
            let rel_path = path.strip_prefix(&fn_dir)?;
            let rel_path_str = rel_path.to_str().unwrap().replace('\\', "/");

            zip.start_file(&rel_path_str, options)?;
            let mut file = fs::File::open(path)?;
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;
            zip.write_all(&buf)?;
        }

        zip.finish()?;
    }

    Ok(())
}

fn rewrite_manifest(src_dir: &Path) -> Result<()> {
    let manifest_path = src_dir.join("manifest.json");
    let manifest_str = fs::read_to_string(&manifest_path)?;
    let mut manifest: BundleManifest = serde_json::from_str(&manifest_str)?;

    // Update migration hashes
    if let Some(ref mut data) = manifest.capabilities.data {
        for migration in &mut data.migrations {
            let file_path = src_dir.join(&migration.path);
            migration.sha256 = compute_sha256(&file_path)?;
        }
    }

    // Update function hashes
    if let Some(ref mut functions) = manifest.capabilities.functions {
        for func in functions {
            let file_path = src_dir.join(&func.path);
            func.sha256 = compute_sha256(&file_path)?;
        }
    }

    // Update job hashes
    if let Some(ref mut jobs) = manifest.capabilities.jobs {
        for job in jobs {
            let file_path = src_dir.join(&job.path);
            job.sha256 = compute_sha256(&file_path)?;
        }
    }

    // Update sites hashes
    if let Some(ref mut sites) = manifest.capabilities.sites {
        for site in sites {
            let file_path = src_dir.join(&site.path);
            site.sha256 = compute_sha256(&file_path)?;
        }
    }

    // Write updated manifest
    let updated = serde_json::to_string_pretty(&manifest)?;
    fs::write(&manifest_path, updated)?;

    println!("Updated manifest with SHA-256 hashes");
    Ok(())
}

fn compute_sha256(path: &Path) -> Result<String> {
    let data = fs::read(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let hash = hasher.finalize();
    Ok(hex::encode(hash))
}
