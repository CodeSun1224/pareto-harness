use std::{
    env, fs,
    path::{Path, PathBuf},
    process,
};

use pareto_protocol::{canonical_json, generate_schema_bundle};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("schemas"));
    publish(&output)
}

fn publish(output: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = output
        .file_name()
        .ok_or("schema output must be a named directory")?
        .to_string_lossy();
    let staging = parent.join(format!(".{name}.staging-{}", process::id()));
    let backup = parent.join(format!(".{name}.backup-{}", process::id()));
    if staging.exists() || backup.exists() {
        return Err("schema staging path already exists".into());
    }
    fs::create_dir(&staging)?;
    if let Err(error) = write_bundle(&staging) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    if output.exists() {
        fs::rename(output, &backup)?;
    }
    if let Err(error) = fs::rename(&staging, output) {
        if backup.exists() {
            let _ = fs::rename(&backup, output);
        }
        return Err(error.into());
    }
    if backup.exists() {
        fs::remove_dir_all(backup)?;
    }
    Ok(())
}

fn write_bundle(output: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let bundle = generate_schema_bundle()?;
    for schema in bundle.schemas {
        let content = format!("{}\n", canonical_json(&schema.document)?);
        fs::write(output.join(schema.filename), content)?;
    }
    fs::write(
        output.join("schema-set-v1.0.manifest.json"),
        format!(
            "{}\n",
            canonical_json(&serde_json::to_value(bundle.manifest)?)?
        ),
    )?;
    fs::write(
        output.join("schema-set-v1.0.ref.json"),
        format!(
            "{}\n",
            canonical_json(&serde_json::to_value(bundle.reference)?)?
        ),
    )?;
    Ok(())
}
