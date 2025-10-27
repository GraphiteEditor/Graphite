use anyhow::{Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "crate-hierarchy-viz")]
#[command(about = "Visualize the crate hierarchy in the Graphite workspace")]
struct Args {
    /// Workspace root directory (defaults to current directory)
    #[arg(short, long)]
    workspace: Option<PathBuf>,

    /// Output format: dot, text
    #[arg(short, long, default_value = "dot")]
    format: String,

    /// Output file (defaults to stdout)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Include external dependencies (workspace dependencies)
    #[arg(long)]
    include_external: bool,

    /// Exclude dyn-any from the graph (it's used everywhere)
    #[arg(long)]
    exclude_dyn_any: bool,
}

#[derive(Debug, Deserialize)]
struct WorkspaceToml {
    workspace: WorkspaceConfig,
}

#[derive(Debug, Deserialize)]
struct WorkspaceConfig {
    members: Vec<String>,
    dependencies: Option<HashMap<String, WorkspaceDependency>>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum WorkspaceDependency {
    Simple(String),
    Detailed {
        path: Option<String>,
        version: Option<String>,
        workspace: Option<bool>,
        #[serde(flatten)]
        other: HashMap<String, toml::Value>,
    },
}

#[derive(Debug, Deserialize)]
struct CrateToml {
    package: PackageConfig,
    dependencies: Option<HashMap<String, CrateDependency>>,
}

#[derive(Debug, Deserialize)]
struct PackageConfig {
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum CrateDependency {
    Simple(String),
    Detailed {
        path: Option<String>,
        workspace: Option<bool>,
        version: Option<String>,
        optional: Option<bool>,
        #[serde(flatten)]
        other: HashMap<String, toml::Value>,
    },
}

#[derive(Debug, Clone)]
struct CrateInfo {
    name: String,
    path: PathBuf,
    dependencies: Vec<String>,
    external_dependencies: Vec<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let workspace_root = args.workspace.unwrap_or_else(|| std::env::current_dir().unwrap());
    let workspace_toml_path = workspace_root.join("Cargo.toml");

    // Parse workspace Cargo.toml
    let workspace_content = fs::read_to_string(&workspace_toml_path)
        .with_context(|| format!("Failed to read {:?}", workspace_toml_path))?;
    let workspace_toml: WorkspaceToml = toml::from_str(&workspace_content)
        .with_context(|| "Failed to parse workspace Cargo.toml")?;

    // Get workspace dependencies (external crates defined at workspace level)
    let workspace_deps: HashSet<String> = workspace_toml
        .workspace
        .dependencies
        .unwrap_or_default()
        .keys()
        .cloned()
        .collect();

    // Parse each member crate and build name mapping
    let mut crates = Vec::new();
    let mut workspace_crate_names = HashSet::new();

    // First pass: collect all workspace crate names
    for member in &workspace_toml.workspace.members {
        let crate_path = workspace_root.join(member);
        let cargo_toml_path = crate_path.join("Cargo.toml");

        if !cargo_toml_path.exists() {
            eprintln!("Warning: Cargo.toml not found for member: {}", member);
            continue;
        }

        let crate_content = fs::read_to_string(&cargo_toml_path)
            .with_context(|| format!("Failed to read {:?}", cargo_toml_path))?;
        let crate_toml: CrateToml = toml::from_str(&crate_content)
            .with_context(|| format!("Failed to parse Cargo.toml for {}", member))?;

        workspace_crate_names.insert(crate_toml.package.name.clone());
    }

    // Second pass: parse dependencies now that we know all workspace crate names
    for member in &workspace_toml.workspace.members {
        let crate_path = workspace_root.join(member);
        let cargo_toml_path = crate_path.join("Cargo.toml");

        if !cargo_toml_path.exists() {
            continue;
        }

        let crate_content = fs::read_to_string(&cargo_toml_path)
            .with_context(|| format!("Failed to read {:?}", cargo_toml_path))?;
        let crate_toml: CrateToml = toml::from_str(&crate_content)
            .with_context(|| format!("Failed to parse Cargo.toml for {}", member))?;

        let mut dependencies = Vec::new();
        let mut external_dependencies = Vec::new();

        if let Some(deps) = &crate_toml.dependencies {
            for (dep_name, dep_config) in deps {
                let is_workspace_crate = workspace_crate_names.contains(dep_name);
                let is_workspace_dep = workspace_deps.contains(dep_name);

                let is_local_dep = match dep_config {
                    CrateDependency::Detailed { workspace: Some(true), .. } => is_workspace_dep,
                    CrateDependency::Detailed { path: Some(_), .. } => true,
                    CrateDependency::Simple(_) => is_workspace_dep,
                    _ => false,
                };

                // Check if this dependency has a different package name
                let actual_dep_name = match dep_config {
                    CrateDependency::Detailed { other, .. } => {
                        // Check if there's a "package" field that renames the dependency
                        if let Some(toml::Value::String(package_name)) = other.get("package") {
                            package_name.clone()
                        } else {
                            dep_name.clone()
                        }
                    }
                    _ => dep_name.clone(),
                };

                let is_actual_workspace_crate = workspace_crate_names.contains(&actual_dep_name);

                if is_workspace_crate || is_actual_workspace_crate || is_local_dep {
                    dependencies.push(actual_dep_name);
                } else {
                    external_dependencies.push(actual_dep_name);
                }
            }
        }

        crates.push(CrateInfo {
            name: crate_toml.package.name.clone(),
            path: crate_path,
            dependencies,
            external_dependencies,
        });
    }

    // Filter dependencies to only include workspace crates
    for crate_info in &mut crates {
        crate_info.dependencies.retain(|dep| workspace_crate_names.contains(dep));
    }

    // Generate output
    let output = match args.format.as_str() {
        "dot" => generate_dot_format(&crates, args.include_external, args.exclude_dyn_any)?,
        "text" => generate_text_format(&crates, args.include_external, args.exclude_dyn_any)?,
        _ => anyhow::bail!("Unsupported format: {}", args.format),
    };

    // Write output
    if let Some(output_path) = args.output {
        fs::write(&output_path, output)
            .with_context(|| format!("Failed to write to {:?}", output_path))?;
        println!("Output written to: {:?}", output_path);
    } else {
        print!("{}", output);
    }

    Ok(())
}

fn generate_dot_format(crates: &[CrateInfo], include_external: bool, exclude_dyn_any: bool) -> Result<String> {
    let mut output = String::new();
    output.push_str("digraph CrateHierarchy {\n");
    output.push_str("    rankdir=LR;\n");
    output.push_str("    node [shape=box, style=\"rounded,filled\", fillcolor=lightblue];\n");
    output.push_str("    edge [color=gray];\n\n");

    // Add subgraphs for different categories
    output.push_str("    subgraph cluster_core {\n");
    output.push_str("        label=\"Core Components\";\n");
    output.push_str("        style=filled;\n");
    output.push_str("        fillcolor=lightgray;\n");

    let core_crates: Vec<_> = crates.iter()
        .filter(|c| c.name.starts_with("graphite-") || c.name == "editor")
        .collect();

    for crate_info in &core_crates {
        output.push_str(&format!("        \"{}\";\n", crate_info.name));
    }
    output.push_str("    }\n\n");

    output.push_str("    subgraph cluster_nodegraph {\n");
    output.push_str("        label=\"Node Graph System\";\n");
    output.push_str("        style=filled;\n");
    output.push_str("        fillcolor=lightyellow;\n");

    let nodegraph_crates: Vec<_> = crates.iter()
        .filter(|c| c.name.starts_with("graphene-") ||
                   c.name == "graph-craft" ||
                   c.name == "interpreted-executor" ||
                   c.name == "wgpu-executor" ||
                   c.name == "node-macro" ||
                   c.name == "preprocessor")
        .collect();

    for crate_info in &nodegraph_crates {
        output.push_str(&format!("        \"{}\";\n", crate_info.name));
    }
    output.push_str("    }\n\n");

    output.push_str("    subgraph cluster_libraries {\n");
    output.push_str("        label=\"Libraries\";\n");
    output.push_str("        style=filled;\n");
    output.push_str("        fillcolor=lightgreen;\n");

    let library_crates: Vec<_> = crates.iter()
        .filter(|c| !c.name.starts_with("graphite-") &&
                   !c.name.starts_with("graphene-") &&
                   c.name != "graph-craft" &&
                   c.name != "interpreted-executor" &&
                   c.name != "wgpu-executor" &&
                   c.name != "node-macro" &&
                   c.name != "preprocessor" &&
                   c.name != "editor")
        .collect();

    for crate_info in &library_crates {
        output.push_str(&format!("        \"{}\";\n", crate_info.name));
    }
    output.push_str("    }\n\n");

    // Add dependencies as edges
    for crate_info in crates {
        for dep in &crate_info.dependencies {
            if exclude_dyn_any && dep == "dyn-any" {
                continue;
            }
            output.push_str(&format!("    \"{}\" -> \"{}\";\n", crate_info.name, dep));
        }

        if include_external {
            for dep in &crate_info.external_dependencies {
                if exclude_dyn_any && dep == "dyn-any" {
                    continue;
                }
                output.push_str(&format!("    \"{}\" -> \"{}\" [style=dashed, color=red];\n", crate_info.name, dep));
            }
        }
    }

    output.push_str("}\n");
    Ok(output)
}

fn generate_text_format(crates: &[CrateInfo], include_external: bool, exclude_dyn_any: bool) -> Result<String> {
    let mut output = String::new();
    output.push_str("Graphite Workspace Crate Hierarchy\n");
    output.push_str("==================================\n\n");

    for crate_info in crates {
        output.push_str(&format!("Crate: {}\n", crate_info.name));
        output.push_str(&format!("Path: {}\n", crate_info.path.display()));

        let filtered_deps: Vec<_> = crate_info.dependencies.iter()
            .filter(|dep| !exclude_dyn_any || *dep != "dyn-any")
            .collect();

        if !filtered_deps.is_empty() {
            output.push_str("Workspace Dependencies:\n");
            for dep in filtered_deps {
                output.push_str(&format!("  - {}\n", dep));
            }
        }

        if include_external {
            let filtered_external_deps: Vec<_> = crate_info.external_dependencies.iter()
                .filter(|dep| !exclude_dyn_any || *dep != "dyn-any")
                .collect();

            if !filtered_external_deps.is_empty() {
                output.push_str("External Dependencies:\n");
                for dep in filtered_external_deps {
                    output.push_str(&format!("  - {}\n", dep));
                }
            }
        }

        output.push_str("\n");
    }

    Ok(output)
}