use std::path::Path;

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use serde_json::{json, Value};

use crate::service::{
    load_service_archetypes_catalog, service_archetype_by_id, BindingKind, IngressKind,
    ServiceArchetype, SERVICE_MANIFEST_SCHEMA_VERSION,
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
    let mut cell_required = vec![
        "cell_key",
        "cell_kind",
        "entry_symbol",
        "ingress_kind",
        "runtime_class",
        "scale_class",
        "binding_refs",
        "topology_group",
    ];
    if matches!(
        archetype.ingress_kind,
        IngressKind::Event | IngressKind::Schedule
    ) {
        cell_required.push("runtime");
    }
    let mut cell_properties = json!({
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
    });
    cell_properties
        .as_object_mut()
        .expect("cell properties object")
        .insert(
            "runtime".to_string(),
            runtime_schema_for_archetype(archetype),
        );

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
            "required": cell_required,
            "properties": cell_properties
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
    let runtime_line = runtime_grammar_line(archetype);
    let runtime_defs = runtime_grammar_defs(archetype);

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
            "  \"topology_group\": \"primary\"{runtime_line}\n",
            "}}\n",
            "<binding> ::= {{ \"name\": <binding-ref>, \"kind\": <binding-kind>, \"required\": <bool> }}\n",
            "<binding-kind> ::= {bindings}\n",
            "{runtime_defs}"
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
        runtime_line = runtime_line,
        bindings = bindings,
        runtime_defs = runtime_defs,
    )
}

fn runtime_schema_for_archetype(archetype: &ServiceArchetype) -> Value {
    let mut properties = serde_json::Map::new();
    if matches!(archetype.ingress_kind, IngressKind::Event) {
        properties.insert(
            "event".to_string(),
            json!({
              "type": "object",
              "additionalProperties": false,
              "required": ["binding_ref", "topic", "consumer_group"],
              "properties": {
                "binding_ref": { "type": "string", "minLength": 1 },
                "topic": { "type": "string", "minLength": 1 },
                "consumer_group": { "type": "string", "minLength": 1 },
                "ack_mode": { "type": "string", "enum": ["auto", "manual"] },
                "max_in_flight": { "type": "integer", "minimum": 1 },
                "drain_timeout_seconds": { "type": "integer", "minimum": 0 }
              }
            }),
        );
    }
    if matches!(archetype.ingress_kind, IngressKind::Schedule) {
        properties.insert(
            "schedule".to_string(),
            json!({
              "type": "object",
              "additionalProperties": false,
              "required": ["cron"],
              "properties": {
                "cron": { "type": "string", "minLength": 1 },
                "timezone": { "type": "string", "minLength": 1 },
                "concurrency_policy": { "type": "string", "enum": ["allow", "forbid", "replace"] },
                "retry_limit": { "type": "integer", "minimum": 0 },
                "start_deadline_seconds": { "type": "integer", "minimum": 0 },
                "suspend": { "type": "boolean" }
              }
            }),
        );
    }
    properties.insert(
        "probes".to_string(),
        json!({
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "startup": probe_schema(),
            "readiness": probe_schema(),
            "liveness": probe_schema()
          }
        }),
    );
    properties.insert(
        "rollout".to_string(),
        json!({
          "type": "object",
          "additionalProperties": false,
          "required": ["strategy"],
          "properties": {
            "strategy": { "type": "string", "enum": ["rolling", "canary-lite", "recreate"] },
            "max_unavailable": { "type": "string", "minLength": 1 },
            "max_surge": { "type": "string", "minLength": 1 },
            "canary_percent": { "type": "integer", "minimum": 1, "maximum": 100 }
          }
        }),
    );
    if matches!(
        archetype.ingress_kind,
        IngressKind::Http | IngressKind::Event
    ) {
        properties.insert(
            "autoscaling".to_string(),
            json!({
              "type": "object",
              "additionalProperties": false,
              "required": ["min_replicas", "max_replicas"],
              "properties": {
                "min_replicas": { "type": "integer", "minimum": 0 },
                "max_replicas": { "type": "integer", "minimum": 1 },
                "target_cpu_utilization": { "type": "integer", "minimum": 1, "maximum": 100 },
                "target_inflight": { "type": "integer", "minimum": 1 }
              }
            }),
        );
    }

    Value::Object(serde_json::Map::from_iter([
        ("type".to_string(), json!("object")),
        ("additionalProperties".to_string(), json!(false)),
        ("properties".to_string(), Value::Object(properties)),
    ]))
}

fn probe_schema() -> Value {
    json!({
      "type": "object",
      "additionalProperties": false,
      "required": ["probe_kind"],
      "properties": {
        "probe_kind": { "type": "string", "enum": ["http", "exec"] },
        "path": { "type": "string", "minLength": 1, "pattern": "^/" },
        "port": { "type": "integer", "minimum": 1, "maximum": 65535 },
        "command": {
          "type": "array",
          "items": { "type": "string", "minLength": 1 }
        },
        "initial_delay_seconds": { "type": "integer", "minimum": 0 },
        "period_seconds": { "type": "integer", "minimum": 0 },
        "timeout_seconds": { "type": "integer", "minimum": 0 },
        "success_threshold": { "type": "integer", "minimum": 1 },
        "failure_threshold": { "type": "integer", "minimum": 1 }
      }
    })
}

fn runtime_grammar_line(archetype: &ServiceArchetype) -> String {
    match archetype.ingress_kind {
        IngressKind::Http => ",\n  \"runtime\": { \"probes\": <probes>?, \"rollout\": <rollout>?, \"autoscaling\": <autoscaling>? }".to_string(),
        IngressKind::Event => ",\n  \"runtime\": { \"event\": <event-runtime>, \"probes\": <probes>?, \"rollout\": <rollout>?, \"autoscaling\": <autoscaling>? }".to_string(),
        IngressKind::Schedule => ",\n  \"runtime\": { \"schedule\": <schedule-runtime>, \"probes\": <probes>? }".to_string(),
        IngressKind::Workflow | IngressKind::Mcp => ",\n  \"runtime\": { \"probes\": <probes>?, \"rollout\": <rollout>? }".to_string(),
    }
}

fn runtime_grammar_defs(archetype: &ServiceArchetype) -> String {
    let mut defs = String::from(
        "<probes> ::= { \"startup\": <probe>?, \"readiness\": <probe>?, \"liveness\": <probe>? }\n\
<probe> ::= { \"probe_kind\": \"http\" | \"exec\", \"path\": \"/healthz\"?, \"command\": [ <string>+ ]?, \"port\": <u16>? }\n\
<rollout> ::= { \"strategy\": \"rolling\" | \"canary-lite\" | \"recreate\", \"max_unavailable\": <string>?, \"max_surge\": <string>?, \"canary_percent\": <u8>? }\n",
    );
    if matches!(archetype.ingress_kind, IngressKind::Event) {
        defs.push_str(
            "<event-runtime> ::= { \"binding_ref\": <binding-ref>, \"topic\": <string>, \"consumer_group\": <string>, \"ack_mode\": \"auto\" | \"manual\"?, \"max_in_flight\": <u32>?, \"drain_timeout_seconds\": <u32>? }\n",
        );
    }
    if matches!(archetype.ingress_kind, IngressKind::Schedule) {
        defs.push_str(
            "<schedule-runtime> ::= { \"cron\": <string>, \"timezone\": <string>?, \"concurrency_policy\": \"allow\" | \"forbid\" | \"replace\"?, \"retry_limit\": <u32>?, \"start_deadline_seconds\": <u32>?, \"suspend\": <bool>? }\n",
        );
    }
    if matches!(
        archetype.ingress_kind,
        IngressKind::Http | IngressKind::Event
    ) {
        defs.push_str(
            "<autoscaling> ::= { \"min_replicas\": <u32>, \"max_replicas\": <u32>, \"target_cpu_utilization\": <u8>?, \"target_inflight\": <u32>? }\n",
        );
    }
    defs
}
