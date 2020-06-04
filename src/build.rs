#![allow(warnings)]

// TODO: Compilation of ./test
// TODO: Avoid rebuilding clean modules
// TODO: Download deps from Hex
// TODO: Support compilation of rebar3 packages
// TODO: Support flexible compiler interface for use by rebar3 + mix
// TODO: Track removed files in src and test so they can be removed from _build
// TODO: test profile and default profile
// TODO: only compile test code in test profile

mod dep_tree;
mod erlang_code_generator;
mod package_analyser;
mod project_analyser;
pub mod project_root;

use crate::{
    ast::TypedModule,
    build::{
        erlang_code_generator::ErlangCodeGenerator, project_analyser::ProjectAnalyser,
        project_root::ProjectRoot,
    },
    config::{self, PackageConfig},
    erl,
    error::{Error, FileIOAction, FileKind, GleamExpect},
    file::{self, OutputFile},
    grammar, parser, typ,
};
use itertools::Itertools;
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs::DirEntry;
use std::path::PathBuf;
use std::process;

pub fn main(root_config: PackageConfig, path: PathBuf) -> Result<HashMap<String, Package>, Error> {
    let root = ProjectRoot::new(path);

    tracing::info!("Copying root package to _build");
    copy_root_package_to_build(&root, &root_config)?;

    tracing::info!("Reading package configs from _build");
    let configs = root.package_configs(&root_config.name)?;

    tracing::info!("Reading and analysing packages");
    let packages = ProjectAnalyser::new(&root, root_config, configs).analyse()?;

    tracing::info!("Generating Erlang source code");
    let compiled_erlang = ErlangCodeGenerator::new(&root, &packages).render();

    tracing::info!("Writing generated Erlang source code to disc");
    file::write_outputs(compiled_erlang.as_slice())?;

    tracing::info!("Compiling Erlang source code to BEAM bytecode");
    compile_erlang_to_beam(&root, &packages)?;

    Ok(packages)
}

#[derive(Debug)]
pub struct Package {
    pub config: PackageConfig,
    pub modules: Vec<Module>,
}

#[derive(Debug)]
pub struct Module {
    pub name: String,
    pub code: String,
    pub path: PathBuf,
    pub origin: Origin,
    pub ast: TypedModule,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Origin {
    Src,
    Test,
}

fn compile_erlang_to_beam(
    root: &ProjectRoot,
    packages: &HashMap<String, Package>,
) -> Result<(), Error> {
    let escript_path = root.build_path().join("compile_escript.erl");
    let escript_source = std::include_str!("build/compile_escript.erl").to_string();

    file::write_output(&OutputFile {
        path: escript_path.clone(),
        text: escript_source,
    })?;

    // Run escript to compile Erlang to beam files
    let mut command = process::Command::new("escript");
    command.arg(escript_path);
    command.arg(root.build_path());

    tracing::trace!("Running OS process {:?}", command);
    let status = command.status().unwrap(); // TODO

    // TODO: check status

    Ok(())
}

fn copy_root_package_to_build(
    root: &ProjectRoot,
    root_config: &PackageConfig,
) -> Result<(), Error> {
    let target = root.default_build_lib_package_path(&root_config.name);
    let path = &root.root;

    // Reset _build dir
    file::delete_dir(&target)?;
    file::mkdir(&target)?;

    // Copy source files across
    file::copy(path.join("gleam.toml"), target.join("gleam.toml"))?;
    file::copy_dir(path.join("src"), &target)?;
    file::copy_dir(path.join("test"), &target)?;

    Ok(())
}