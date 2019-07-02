#[macro_use]
extern crate clap;
#[macro_use]
extern crate log;

use futures::future::Future;
mod config;

fn main() {
    let args = clap_app!(sprinkler =>
            (version: crate_version!())
            (author: crate_authors!())
            (about: crate_description!())
            (@arg VERBOSE: --verbose -v ... "Logging verbosity")
        ).get_matches();
    config::setup_logger(args.occurrences_of("VERBOSE")).expect("Logger Error.");

    tokio::run(futures::future::lazy(|| {
        let sprinklers: Vec<Box<dyn Sprinkler>> = config::get_sprinklers();
        let switch = Switch::new();
        switch.connect_all(&sprinklers);
        let addr = "0.0.0.0:3777".parse().unwrap();
        sprinkler_api::server(&addr, &switch);
        Ok(())
    }));
}
