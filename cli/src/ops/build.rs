//! build command

use clap::{App, Arg, ArgMatches, SubCommand};
use core::errors::Result;
use core::{Filesystem, Reporter};
use env;
use utils::{session, load_manifest};
use core::model::Language;

pub fn options<'a, 'b>() -> App<'a, 'b> {
    let out = SubCommand::with_name("build").about("Build specifications");

    let out = out.arg(
        Arg::with_name("lang")
            .long("lang")
            .takes_value(true)
            .help("Language to build for"),
    );

    let out = out.arg(
        Arg::with_name("list-modules")
            .long("list-modules")
            .help("List available modules and their corresponding configurations"),
    );

    out
}

pub fn entry(fs: &Filesystem, reporter: &mut Reporter, matches: &ArgMatches) -> Result<()> {

    let mut done = false;

    if matches.is_present("list-modules") {
        //list_modules()?;

        match Language {
            Csharp => println!("csharp"),
            Go => println!("go"),
            Java => println!("java"),
            JavaScript => println!("js"),
            Json => println!("json"),
            OpenApi => println!("openapi"),
            Python => println!("python"),
            Python3 => println!("python3"),
            Reproto => println!("reproto"),
            Rust => println!("rust"),
            Swift => println!("swift")
        }

        done = true;
    }

    if done {
        return Ok(())
    }

    let manifest = load_manifest(matches)?;
    let lang = manifest.lang().ok_or_else(|| {
        "no language to build for, either specify in manifest under `language` or `--lang`"
    })?;

    let mut resolver = env::resolver(&manifest)?;
    let handle = fs.open_root(manifest.output.as_ref().map(AsRef::as_ref))?;
    let session = session(lang.copy(), &manifest, reporter, resolver.as_mut())?;
    lang.compile(handle.as_ref(), session, manifest)?;
    Ok(())
}


