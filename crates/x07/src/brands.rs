use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde_json::Value;
use x07_worlds::WorldId;

#[derive(Debug, Clone)]
struct BrandModule {
    imports: Vec<String>,
    validators: BTreeMap<String, String>,
}

fn load_brand_module<'a>(
    module_roots: &[PathBuf],
    module_id: &str,
    cache: &'a mut BTreeMap<String, BrandModule>,
) -> Result<&'a BrandModule> {
    if !cache.contains_key(module_id) {
        let source =
            x07c::module_source::load_module_source(module_id, WorldId::SolvePure, module_roots)
                .map_err(|err| anyhow::anyhow!(err.message.to_string()))?;
        let doc: Value = serde_json::from_str(&source.src)
            .with_context(|| format!("parse module JSON for {module_id:?}"))?;
        let imports = doc
            .get("imports")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let meta = doc
            .get("meta")
            .and_then(Value::as_object)
            .map(|obj| {
                obj.iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect::<BTreeMap<_, _>>()
            })
            .unwrap_or_default();
        let validators = x07c::stream_pipe::brand_registry_from_meta_v1(&meta)
            .map_err(|err| anyhow::anyhow!(err.message.to_string()))?;
        cache.insert(
            module_id.to_string(),
            BrandModule {
                imports,
                validators,
            },
        );
    }
    Ok(cache.get(module_id).expect("brand module inserted"))
}

pub(crate) fn resolve_brand_validator(
    module_roots: &[PathBuf],
    entry: &str,
    brand_id: &str,
) -> Result<String> {
    let Some(validator) = try_resolve_brand_validator(module_roots, entry, brand_id)? else {
        anyhow::bail!(
            "brand {:?} is missing meta.brands_v1.validate in the reachable module graph for {:?}",
            brand_id,
            entry
        );
    };
    Ok(validator)
}

pub(crate) fn try_resolve_brand_validator(
    module_roots: &[PathBuf],
    entry: &str,
    brand_id: &str,
) -> Result<Option<String>> {
    let (module_id, _) = entry.rsplit_once('.').context("symbol must contain '.'")?;
    let mut cache = BTreeMap::new();
    let mut queue = VecDeque::from([module_id.to_string()]);
    let mut visited = BTreeSet::new();
    let mut matches = BTreeSet::new();

    while let Some(current) = queue.pop_front() {
        if !visited.insert(current.clone()) {
            continue;
        }
        let module = load_brand_module(module_roots, &current, &mut cache)?;
        if let Some(validator) = module.validators.get(brand_id) {
            matches.insert(validator.clone());
        }
        for import in &module.imports {
            queue.push_back(import.clone());
        }
    }

    match matches.len() {
        1 => Ok(Some(
            matches
                .into_iter()
                .next()
                .expect("single validator match"),
        )),
        0 => Ok(None),
        _ => anyhow::bail!(
            "brand {:?} resolves to multiple validators in the reachable module graph for {:?}: {:?}",
            brand_id,
            entry,
            matches.into_iter().collect::<Vec<_>>()
        ),
    }
}
