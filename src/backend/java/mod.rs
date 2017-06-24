mod models;
mod builder;
mod constructor_properties;
mod fasterxml;
mod java_backend;
mod java_options;
mod java_compiler;
mod listeners;
mod lombok;
mod mutable;
mod nullable;

use backend::*;
use clap::{App, ArgMatches};
pub(crate) use codeviz::java::*;
pub(crate) use errors::*;
use options::Options;
use self::java_backend::*;
use self::java_compiler::*;
use self::java_options::*;
use self::listeners::*;
pub(crate) use self::models::*;

pub const JAVA_CONTEXT: &str = "java";

fn setup_module(module: &str) -> Result<Box<Listeners>> {
    let module: Box<Listeners> = match module {
        "builder" => Box::new(builder::Module::new()),
        "constructor_properties" => Box::new(constructor_properties::Module::new()),
        "fasterxml" => Box::new(fasterxml::Module::new()),
        "lombok" => Box::new(lombok::Module::new()),
        "mutable" => Box::new(mutable::Module::new()),
        "nullable" => Box::new(nullable::Module::new()),
        _ => return Err(format!("No such module: {}", module).into()),
    };

    Ok(module)
}

pub fn setup_listeners(options: Options) -> Result<(JavaOptions, Box<Listeners>)> {
    let mut listeners: Vec<Box<Listeners>> = Vec::new();

    for module in &options.modules {
        listeners.push(setup_module(module)?);
    }

    let mut options = JavaOptions::new();

    for listener in &listeners {
        listener.configure(&mut options)?;
    }

    Ok((options, Box::new(listeners)))
}

pub fn compile_options<'a, 'b>(out: App<'a, 'b>) -> App<'a, 'b> {
    out.about("Compile for Java")
}

pub fn verify_options<'a, 'b>(out: App<'a, 'b>) -> App<'a, 'b> {
    out.about("Verify for Java")
}

pub fn compile(env: Environment,
               options: Options,
               compiler_options: CompilerOptions,
               _matches: &ArgMatches)
               -> Result<()> {
    let (options, listeners) = setup_listeners(options)?;
    let backend = JavaBackend::new(env, options, listeners);
    let compiler = backend.compiler(compiler_options)?;
    compiler.compile()
}

pub fn verify(env: Environment, options: Options, _matches: &ArgMatches) -> Result<()> {
    let (options, listeners) = setup_listeners(options)?;
    let backend = JavaBackend::new(env, options, listeners);
    backend.verify()
}
