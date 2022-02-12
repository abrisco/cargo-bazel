use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, Context, Result};
use cfg_expr::targets::{get_builtin_target_by_triple, TargetInfo};
use cfg_expr::{Expression, Predicate};

use crate::context::CrateContext;
use crate::utils::starlark::Select;

pub fn resolve_cfg_platforms(
    crates: Vec<&CrateContext>,
    supported_platform_triples: &BTreeSet<String>,
) -> Result<BTreeMap<String, BTreeSet<String>>> {
    // Collect all unique configurations from all dependencies into a single set
    let configurations: BTreeSet<String> = crates
        .iter()
        .flat_map(|ctx| {
            let attr = &ctx.common_attrs;
            attr.deps
                .configurations()
                .into_iter()
                .chain(attr.deps_dev.configurations().into_iter())
                .chain(attr.proc_macro_deps.configurations().into_iter())
                .chain(attr.proc_macro_deps_dev.configurations().into_iter())
                // Chain the build dependencies if some are defined
                .chain(if let Some(attr) = &ctx.build_script_attrs {
                    attr.deps
                        .configurations()
                        .into_iter()
                        .chain(attr.proc_macro_deps.configurations().into_iter())
                        .collect::<BTreeSet<Option<&String>>>()
                        .into_iter()
                } else {
                    BTreeSet::new().into_iter()
                })
                .flatten()
        })
        .cloned()
        .collect();

    // Generate target information for each triple string
    let target_infos = supported_platform_triples
        .iter()
        .map(|t| match get_builtin_target_by_triple(t) {
            Some(info) => Ok(info),
            None => Err(anyhow!(
                "Invalid platform triple in supported platforms: {}",
                t
            )),
        })
        .collect::<Result<Vec<&'static TargetInfo>>>()?;

    configurations
        .into_iter()
        // `cfg-expr` requires that the expressions be actual `cfg` expressions. Any time
        // there's a target triple (which is a valid constraint), convert it to a cfg expression.
        .map(|cfg| match cfg.starts_with("cfg(") {
            true => cfg.to_string(),
            false => format!("cfg(target = \"{}\")", cfg),
        })
        // Check the current configuration with against each supported triple
        .map(|cfg| {
            let expression = Expression::parse(&cfg)
                .context(format!("Failed to parse expression: '{}'", cfg))?;

            let triples = target_infos
                .iter()
                .filter(|info| {
                    expression.eval(|p| match p {
                        Predicate::Target(tp) => tp.matches(**info),
                        Predicate::KeyValue { key, val } => {
                            *key == "target" && val == &info.triple.as_str()
                        }
                        // For now there is no other kind of matching
                        _ => false,
                    })
                })
                .map(|info| info.triple.to_string())
                .collect();

            Ok((cfg, triples))
        })
        .collect()
}
