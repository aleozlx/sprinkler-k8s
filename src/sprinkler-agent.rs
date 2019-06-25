#[macro_use]
extern crate clap;
#[macro_use]
extern crate log;

use sprinkler_api::*;
mod docker_oom;
use docker_oom::DockerOOM;
mod config;

fn main() {
    let args = clap_app!(sprinkler =>
            (version: crate_version!())
            (author: crate_authors!())
            (about: crate_description!())
            (@arg VERBOSE: --verbose -v ... "Logging verbosity")
        ).get_matches();
    
    config::setup_logger(args.occurrences_of("VERBOSE")).expect("Logger Error.");

    const MASTER_ADDR: &str = "bridge.dsa.lan:3777";
    let mut builder = SprinklerBuilder::new(SprinklerOptions{ master_addr: String::from(MASTER_ADDR), ..Default::default() });

    // parse FNAME_CONFIG and add triggers
    let sprinklers: Vec<Box<dyn Sprinkler>> = vec![
        Box::new(builder.build::<CommCheck>(String::from("k-prod-cpu-1.dsa.lan"))),
        Box::new(builder.build::<CommCheck>(String::from("k-prod-cpu-2.dsa.lan"))),
        Box::new(builder.build::<CommCheck>(String::from("k-prod-cpu-3.dsa.lan"))),
        Box::new(builder.build::<CommCheck>(String::from("k-prod-cpu-4.dsa.lan"))),
        Box::new(builder.build::<CommCheck>(String::from("k-prod-cpu-5.dsa.lan"))),
        Box::new(builder.build::<CommCheck>(String::from("k-prod-cpu-6.dsa.lan"))),
        Box::new(builder.build::<CommCheck>(String::from("k-prod-cpu-7.dsa.lan"))),
        Box::new(builder.build::<CommCheck>(String::from("k-prod-cpu-8.dsa.lan"))),
        Box::new(builder.build::<CommCheck>(String::from("k-prod-cpu-9.dsa.lan"))),

        // Box::new(builder.build::<DockerOOM>(String::from("k-prod-cpu-1.dsa.lan"))),
        // Box::new(builder.build::<DockerOOM>(String::from("k-prod-cpu-2.dsa.lan"))),
        // Box::new(builder.build::<DockerOOM>(String::from("k-prod-cpu-3.dsa.lan"))),
        // Box::new(builder.build::<DockerOOM>(String::from("k-prod-cpu-4.dsa.lan"))),
        Box::new(builder.build::<DockerOOM>(String::from("k-prod-cpu-5.dsa.lan"))),
        // Box::new(builder.build::<DockerOOM>(String::from("k-prod-cpu-6.dsa.lan"))),
        // Box::new(builder.build::<DockerOOM>(String::from("k-prod-cpu-7.dsa.lan"))),
        // Box::new(builder.build::<DockerOOM>(String::from("k-prod-cpu-8.dsa.lan"))),
        // Box::new(builder.build::<DockerOOM>(String::from("k-prod-cpu-9.dsa.lan")))
    ];

    sprinkler_api::agent(&sprinklers);
    sprinkler_api::loop_forever();
}
