extern crate reproto;
#[macro_use]
extern crate log;
extern crate getopts;

use std::path::Path;
use std::env;

use reproto::errors::*;

use reproto::backends;
use reproto::logger;
use reproto::options::Options;
use reproto::parser::ast;
use reproto::environment::Environment;

fn setup_opts() -> getopts::Options {
    let mut opts = getopts::Options::new();

    opts.reqopt("b",
                "backend",
                "Backend to used to emit code (required).",
                "<backend>");

    opts.optmulti("",
                  "path",
                  "Paths to look for definitions. Can be used multiple times.",
                  "<dir>");

    opts.reqopt("o", "out", "Path to write output to (required).", "<dir>");

    opts.optflag("h", "help", "Print help.");

    opts.optflag("", "debug", "Enable debug logging.");

    opts.optflag("r",
                 "recursive",
                 "Process the arguments recursively (looking for .reproto files).");

    opts.optopt("",
                "package-prefix",
                "Package prefix to use when generating classes.",
                "<package>");

    opts
}

fn print_usage(program: &str, opts: getopts::Options) {
    let brief = format!("Usage: {} [options]", program);
    println!("hello: {}", opts.usage(&brief));
}

/// Configure logging
///
/// If debug (--debug) is specified, logging should be configured with LogLevelFilter::Debug.
fn setup_logger(matches: &getopts::Matches) -> Result<()> {
    let level: log::LogLevelFilter = match matches.opt_present("debug") {
        true => log::LogLevelFilter::Debug,
        false => log::LogLevelFilter::Info,
    };

    logger::init(level)?;

    Ok(())
}

fn entry() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let opts = setup_opts();

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            print_usage(&args[0], opts);
            return Err(f.into());
        }
    };

    if matches.opt_present("h") {
        print_usage(&args[0], opts);
        return Ok(());
    }

    if matches.free.len() < 1 {
        print_usage(&args[0], opts);
        return Ok(());
    }

    setup_logger(&matches)?;

    let backend = matches.opt_str("backend").ok_or("--backend <backend> is required")?;
    let backend = backends::resolve(&backend)?;

    let out_path = matches.opt_str("out").ok_or("--out <dir> is required")?;
    let out_path = Path::new(&out_path);

    let package_prefix = matches.opt_str("package-prefix");

    let paths = matches.opt_strs("path").iter().map(Path::new).map(ToOwned::to_owned).collect();

    let options = Options {
        out_path: out_path.to_path_buf(),
        package_prefix: package_prefix,
    };

    let mut env = Environment::new(paths);

    for package in matches.free {
        let package = ast::Package::new(package.split(".").map(ToOwned::to_owned).collect());
        env.import(&package)?;
    }

    backend.process(&options, &env)?;
    Ok(())
}

fn main() {
    match entry() {
        Err(e) => {
            error!("{}", e);

            for e in e.iter().skip(1) {
                error!("  caused by: {}", e);
            }

            if let Some(backtrace) = e.backtrace() {
                error!("  backtrace: {:?}", backtrace);
            }

            ::std::process::exit(1);
        }
        _ => {}
    };

    ::std::process::exit(0);
}
