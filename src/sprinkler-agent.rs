#[macro_use]
extern crate clap;
#[macro_use]
extern crate log;

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
        sprinkler_api::agent(&sprinklers);
        Ok(())
    }));

    // Prevent the agent from shutting down when there are no async tasks.
    sprinkler_api::loop_forever();
}
