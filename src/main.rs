//! Written by AI.

use cargo_metadata::MetadataCommand;
use clap::Parser;
use colored::Colorize;
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::Builder;

#[derive(Parser)]
#[command(name = "cargo")]
#[command(bin_name = "cargo")]
enum CargoCli {
    SizeOf(SizeOfArgs),
}

#[derive(Parser, Debug)]
struct SizeOfArgs {
    /// The type to measure (e.g., "MyStruct" or "Vec<u8>")
    type_name: String,

    /// Package to inspect (required in workspaces if not in a member directory)
    #[arg(short, long)]
    package: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let error = "error".bright_red();

    // Cargo passes the subcommand name as the first argument, so we parse it nested
    let CargoCli::SizeOf(args) = CargoCli::parse();

    // 1. Fetch Metadata
    let metadata = MetadataCommand::new().exec()?;

    // 2. Determine target package
    let package = if let Some(pkg_name) = args.package {
        let Some(package) = metadata.packages.iter().find(|p| p.name == pkg_name) else {
            eprintln!(
                "{error}: package `{pkg_name}` not found in workspace `{}`",
                metadata.workspace_root,
            );
            return Ok(());
        };

        package
    } else {
        // Fallback to the package in the current working directory
        let Some(package) = metadata.root_package() else {
            eprintln!(
                "{error}: `cargo size-of` could not determine which package to modify. Use the `--package` option to specify a package.\navailable packages: {}",
                metadata
                    .workspace_default_packages()
                    .iter()
                    .map(|pkg| &**pkg.name)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            return Ok(());
        };

        package
    };

    if !package.targets.iter().any(|target| target.is_lib()) {
        eprintln!("{error}: a lib target must be available for `cargo size-of`");
        return Ok(());
    }

    let pkg_dir = package.manifest_path.parent().unwrap();
    let examples_dir = pkg_dir.join("examples");
    let exists = fs::exists(&examples_dir)?;
    fs::create_dir_all(&examples_dir)?;

    // 3. Create a temporary example file
    // We use a prefix so it's clearly a temp file
    let mut temp_file = Builder::new()
        .prefix("size_check_")
        .suffix(".rs")
        .tempfile_in(&examples_dir)?;

    let code = format!(
        "
        #![allow(warnings)]
        use {}::*; \n\
        fn main() {{ \n\
            println!(\"{{:<15}} {{}}\", \"Type:\", \"{}\"); \n\
            println!(\"{{:<15}} {{}} bytes\", \"Size:\", std::mem::size_of::<{}>()); \n\
            println!(\"{{:<15}} {{}} bytes\", \"Alignment:\", std::mem::align_of::<{}>()); \n\
        }}",
        package.name.replace('-', "_"), // Crate names in code use underscores
        args.type_name,
        args.type_name,
        args.type_name
    );

    temp_file.write_all(code.as_bytes())?;

    // Get the filename without extension for `cargo run --example`
    let example_name = temp_file.path().file_stem().unwrap().to_str().unwrap();

    // 4. Execute Cargo Run
    let output = Command::new("cargo")
        .arg("run")
        .arg("--quiet")
        .arg("--color=always")
        .arg("--example")
        .arg(example_name)
        .arg("-p")
        .arg(&*package.name)
        .stdout(Stdio::inherit())
        .output()?;

    let status = output.status;

    if !status.success() {
        eprint!("{}", String::from_utf8(output.stderr)?);
    }

    drop(temp_file);

    if !exists {
        fs::remove_dir(&examples_dir)?;
    }

    Ok(())
}
