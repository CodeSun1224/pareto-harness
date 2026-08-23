use std::{
    env, fs,
    path::{Path, PathBuf},
    process,
};

use pareto_protocol::{GeneratedSchemaBundle, canonical_json, generate_schema_bundle};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("schemas"));
    publish(&output)
}

fn publish(output: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let bundle = generate_schema_bundle()?;
    let digest = bundle
        .reference
        .manifest_digest
        .as_str()
        .strip_prefix("sha256:")
        .ok_or("manifest digest is not sha256")?;
    let sets = output.join("sets");
    fs::create_dir_all(&sets)?;
    let target = sets.join(format!("sha256-{digest}"));
    let staging = sets.join(format!(".staging-sha256-{digest}-{}", process::id()));
    if staging.exists() {
        return Err("schema staging path already exists".into());
    }
    fs::create_dir(&staging)?;
    if let Err(error) = write_bundle(&staging, &bundle) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    if target.exists() {
        let equal = directories_equal(&staging, &target)?;
        fs::remove_dir_all(&staging)?;
        if !equal {
            return Err("existing content-addressed schema set differs byte-for-byte".into());
        }
        return Ok(());
    }
    match fs::rename(&staging, &target) {
        Ok(()) => Ok(()),
        Err(_error) if target.exists() => {
            let equal = directories_equal(&staging, &target)?;
            fs::remove_dir_all(&staging)?;
            if equal {
                Ok(())
            } else {
                Err("concurrent content-addressed publication differs byte-for-byte".into())
            }
        }
        Err(error) => Err(error.into()),
    }
}

fn write_bundle(
    output: &Path,
    bundle: &GeneratedSchemaBundle,
) -> Result<(), Box<dyn std::error::Error>> {
    for schema in &bundle.schemas {
        let content = format!("{}\n", canonical_json(&schema.document)?);
        fs::write(output.join(&schema.filename), content)?;
    }
    fs::write(
        output.join("schema-set-v1.0.manifest.json"),
        format!(
            "{}\n",
            canonical_json(&serde_json::to_value(&bundle.manifest)?)?
        ),
    )?;
    fs::write(
        output.join("schema-set-v1.0.ref.json"),
        format!(
            "{}\n",
            canonical_json(&serde_json::to_value(&bundle.reference)?)?
        ),
    )?;
    Ok(())
}

fn directories_equal(left: &Path, right: &Path) -> Result<bool, Box<dyn std::error::Error>> {
    let mut names = Vec::new();
    for entry in fs::read_dir(left)? {
        names.push(entry?.file_name());
    }
    let mut right_names = Vec::new();
    for entry in fs::read_dir(right)? {
        right_names.push(entry?.file_name());
    }
    names.sort();
    right_names.sort();
    if names != right_names {
        return Ok(false);
    }
    for name in names {
        if fs::read(left.join(&name))? != fs::read(right.join(name))? {
            return Ok(false);
        }
    }
    Ok(true)
}
