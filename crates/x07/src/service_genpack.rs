use std::path::Path;

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use serde_json::{json, Value};

use crate::service::{
    load_service_archetypes_catalog, service_archetype_by_id, BindingKind, ServiceArchetype,
    SERVICE_MANIFEST_SCHEMA_VERSION,
};

#[derive(Debug, Clone, Args)]
pub struct ServiceGenpackArgs {
    #[command(subcommand)]
    pub cmd: Option<ServiceGenpackCommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ServiceGenpackCommand {
    /// Emit an archetype-specific JSON Schema for service generation.
    Schema(ServiceGenpackSchemaArgs),
    /// Emit an archetype-specific CFG-style grammar for service generation.
    Grammar(ServiceGenpackGrammarArgs),
}

#[derive(Debug, Clone, Args)]
pub struct ServiceGenpackSchemaArgs {
    #[arg(long, value_name = "ARCHETYPE")]
    pub archetype: String,
}

#[derive(Debug, Clone, Args)]
pub struct ServiceGenpackGrammarArgs {
    #[arg(long, value_name = "ARCHETYPE")]
    pub archetype: String,
}

pub fn cmd_service_genpack(
    machine: &crate::reporting::MachineArgs,
    args: ServiceGenpackArgs,
) -> Result<std::process::ExitCode> {
    let Some(cmd) = args.cmd else {
        anyhow::bail!("missing service genpack subcommand (try --help)");
    };
    match cmd {
        ServiceGenpackCommand::Schema(args) => cmd_service_genpack_schema(machine, args),
        ServiceGenpackCommand::Grammar(args) => cmd_service_genpack_grammar(machine, args),
    }
}

fn cmd_service_genpack_schema(
    machine: &crate::reporting::MachineArgs,
    args: ServiceGenpackSchemaArgs,
) -> Result<std::process::ExitCode> {
    let catalog = load_service_archetypes_catalog()?;
    let archetype = service_archetype_by_id(&catalog, &args.archetype)
        .with_context(|| format!("unknown service archetype {:?}", args.archetype))?;
    let schema = build_archetype_schema(archetype);
    write_json_output(machine.out.as_deref(), &schema)?;
    Ok(std::process::ExitCode::SUCCESS)
}

fn cmd_service_genpack_grammar(
    machine: &crate::reporting::MachineArgs,
    args: ServiceGenpackGrammarArgs,
) -> Result<std::process::ExitCode> {
    let catalog = load_service_archetypes_catalog()?;
    let archetype = service_archetype_by_id(&catalog, &args.archetype)
        .with_context(|| format!("unknown service archetype {:?}", args.archetype))?;
    let grammar = build_archetype_grammar(archetype);
    if let Some(path) = machine.out.as_deref() {
        crate::reporting::write_bytes(path, grammar.as_bytes())?;
    } else {
        print!("{grammar}");
    }
    Ok(std::process::ExitCode::SUCCESS)
}

fn write_json_output(path: Option<&Path>, value: &Value) -> Result<()> {
    let out = serde_json::to_string_pretty(value)? + "\n";
    if let Some(path) = path {
        crate::reporting::write_bytes(path, out.as_bytes())?;
    } else {
        print!("{out}");
    }
    Ok(())
}

fn build_archetype_schema(archetype: &ServiceArchetype) -> Value {
    let supported_bindings: Vec<_> = archetype
        .supported_bindings
        .iter()
        .map(BindingKind::as_str)
        .collect();
    let package_defaults: Vec<_> = archetype
        .default_packages
        .iter()
        .map(|pkg| json!({"name": pkg.name, "version": pkg.version}))
        .collect();

    json!({
      "$schema": "https://json-schema.org/draft/2020-12/schema",
      "$id": format!("https://x07.io/spec/x07.service.genpack.{}.schema.json", archetype.id),
      "title": format!("x07 service archetype {}", archetype.id),
      "description": archetype.summary,
      "type": "object",
      "additionalProperties": false,
      "required": [
        "schema_version",
        "service_id",
        "display_name",
        "domain_pack",
        "cells"
      ],
      "properties": {
        "schema_version": { "const": SERVICE_MANIFEST_SCHEMA_VERSION },
        "service_id": {
          "type": "string",
          "minLength": 1,
          "pattern": "^[a-z0-9][a-z0-9._-]*$"
        },
        "display_name": {
          "type": "string",
          "minLength": 1,
          "maxLength": 256
        },
        "domain_pack": {
          "type": "object",
          "additionalProperties": false,
          "required": ["id", "display_name"],
          "properties": {
            "id": { "type": "string", "minLength": 1 },
            "display_name": { "type": "string", "minLength": 1 }
          }
        },
        "cells": {
          "type": "array",
          "minItems": 1,
          "maxItems": 1,
          "items": {
            "type": "object",
            "additionalProperties": false,
            "required": [
              "cell_key",
              "cell_kind",
              "entry_symbol",
              "ingress_kind",
              "runtime_class",
              "scale_class",
              "binding_refs",
              "topology_group"
            ],
            "properties": {
              "cell_key": { "type": "string", "default": "primary" },
              "cell_kind": { "const": archetype.cell_kind.as_str() },
              "entry_symbol": { "type": "string", "minLength": 1, "default": "example.main" },
              "ingress_kind": { "const": archetype.ingress_kind.as_str() },
              "runtime_class": {
                "type": "string",
                "enum": [archetype.runtime_class.as_str()]
              },
              "scale_class": {
                "type": "string",
                "enum": [archetype.scale_class.as_str()]
              },
              "binding_refs": {
                "type": "array",
                "items": { "type": "string", "minLength": 1 }
              },
              "topology_group": { "type": "string", "default": "primary" }
            }
          }
        },
        "topology_profiles": {
          "type": "array",
          "items": {
            "type": "object",
            "additionalProperties": false,
            "required": ["id", "placement"],
            "properties": {
              "id": { "type": "string", "minLength": 1 },
              "target_kind": {
                "type": "string",
                "enum": ["hosted", "k8s", "wasmcloud", "oss_remote"]
              },
              "placement": {
                "type": "string",
                "enum": ["co-located", "split-by-cell", "dedicated-cell"]
              },
              "notes": { "type": "string" }
            }
          }
        },
        "resource_bindings": {
          "type": "array",
          "items": {
            "type": "object",
            "additionalProperties": false,
            "required": ["name", "kind"],
            "properties": {
              "name": { "type": "string", "minLength": 1 },
              "kind": { "type": "string", "enum": supported_bindings },
              "required": { "type": "boolean", "default": true },
              "notes": { "type": "string" }
            }
          }
        },
        "default_trust_profile": { "type": "string", "default": "sandboxed_service_v1" }
      },
      "x-x07": {
        "archetype": archetype.id,
        "example_path": archetype.example_path,
        "scaffold_only": archetype.scaffold_only,
        "default_capabilities": archetype.default_capabilities,
        "default_packages": package_defaults
      }
    })
}

fn build_archetype_grammar(archetype: &ServiceArchetype) -> String {
    let bindings = archetype
        .supported_bindings
        .iter()
        .map(|binding| format!("\"{}\"", binding.as_str()))
        .collect::<Vec<_>>()
        .join(" | ");
    let capabilities = archetype.default_capabilities.join(", ");
    let packages = archetype
        .default_packages
        .iter()
        .map(|pkg| format!("{}@{}", pkg.name, pkg.version))
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        concat!(
            "# x07 service genpack grammar for {id}\n",
            "# summary: {summary}\n",
            "# example: {example}\n",
            "# default_capabilities: {capabilities}\n",
            "# default_packages: {packages}\n",
            "<service-manifest> ::= {{\n",
            "  \"schema_version\": \"{schema}\",\n",
            "  \"service_id\": <service-id>,\n",
            "  \"display_name\": <display-name>,\n",
            "  \"domain_pack\": <domain-pack>,\n",
            "  \"cells\": [ <cell> ],\n",
            "  \"topology_profiles\": [ <topology-profile>* ],\n",
            "  \"resource_bindings\": [ <binding>* ],\n",
            "  \"default_trust_profile\": <trust-profile>\n",
            "}}\n",
            "<cell> ::= {{\n",
            "  \"cell_key\": \"primary\",\n",
            "  \"cell_kind\": \"{cell_kind}\",\n",
            "  \"entry_symbol\": <entry-symbol>,\n",
            "  \"ingress_kind\": \"{ingress}\",\n",
            "  \"runtime_class\": \"{runtime}\",\n",
            "  \"scale_class\": \"{scale}\",\n",
            "  \"binding_refs\": [ <binding-ref>* ],\n",
            "  \"topology_group\": \"primary\"\n",
            "}}\n",
            "<binding> ::= {{ \"name\": <binding-ref>, \"kind\": <binding-kind>, \"required\": <bool> }}\n",
            "<binding-kind> ::= {bindings}\n"
        ),
        id = archetype.id,
        summary = archetype.summary,
        example = archetype.example_path,
        capabilities = capabilities,
        packages = packages,
        schema = SERVICE_MANIFEST_SCHEMA_VERSION,
        cell_kind = archetype.cell_kind.as_str(),
        ingress = archetype.ingress_kind.as_str(),
        runtime = archetype.runtime_class.as_str(),
        scale = archetype.scale_class.as_str(),
        bindings = bindings,
    )
}
