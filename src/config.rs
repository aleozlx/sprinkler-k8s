use sprinkler_api::{Sprinkler, SprinklerBuilder, SprinklerOptions, CommCheck};
use crate::docker_oom::DockerOOM;

// pub const FNAME_CONFIG: &str = "/etc/sprinkler.conf.d/config.toml";

pub fn setup_logger(verbose: u64) -> Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{} {} {}",
                chrono::Local::now().format("[%Y-%m-%d %H:%M:%S]"),
                record.level(),
                message
            ))
        })
        .level(match verbose {
            0 => log::LevelFilter::Info,
            1 => log::LevelFilter::Info,
            2 => log::LevelFilter::Debug,
            3 => log::LevelFilter::Trace,
            _ => log::LevelFilter::Trace
        })
        .chain(std::io::stderr())
        .apply()?;
    Ok(())
}

// pub fn get_sprinklers() -> Vec<Box<dyn Sprinkler>> {
//     const MASTER_ADDR: &str = "bridge.dsa.lan:3777";
//     let mut builder = SprinklerBuilder::new(SprinklerOptions{ master_addr: String::from(MASTER_ADDR), ..Default::default() });

//     // parse FNAME_CONFIG and add triggers
//     let sprinklers: Vec<Box<dyn Sprinkler>> = vec![
//         Box::new(builder.build::<CommCheck>(String::from("k-prod-cpu-1.dsa.lan"))),
//         Box::new(builder.build::<CommCheck>(String::from("k-prod-cpu-2.dsa.lan"))),
//         Box::new(builder.build::<CommCheck>(String::from("k-prod-cpu-3.dsa.lan"))),
//         Box::new(builder.build::<CommCheck>(String::from("k-prod-cpu-4.dsa.lan"))),
//         Box::new(builder.build::<CommCheck>(String::from("k-prod-cpu-5.dsa.lan"))),
//         Box::new(builder.build::<CommCheck>(String::from("k-prod-cpu-6.dsa.lan"))),
//         Box::new(builder.build::<CommCheck>(String::from("k-prod-cpu-7.dsa.lan"))),
//         Box::new(builder.build::<CommCheck>(String::from("k-prod-cpu-8.dsa.lan"))),
//         Box::new(builder.build::<CommCheck>(String::from("k-prod-cpu-9.dsa.lan"))),

//         Box::new(builder.build::<DockerOOM>(String::from("k-prod-cpu-1.dsa.lan"))),
//         Box::new(builder.build::<DockerOOM>(String::from("k-prod-cpu-2.dsa.lan"))),
//         Box::new(builder.build::<DockerOOM>(String::from("k-prod-cpu-3.dsa.lan"))),
//         Box::new(builder.build::<DockerOOM>(String::from("k-prod-cpu-4.dsa.lan"))),
//         Box::new(builder.build::<DockerOOM>(String::from("k-prod-cpu-5.dsa.lan"))),
//         Box::new(builder.build::<DockerOOM>(String::from("k-prod-cpu-6.dsa.lan"))),
//         Box::new(builder.build::<DockerOOM>(String::from("k-prod-cpu-7.dsa.lan"))),
//         Box::new(builder.build::<DockerOOM>(String::from("k-prod-cpu-8.dsa.lan"))),
//         Box::new(builder.build::<DockerOOM>(String::from("k-prod-cpu-9.dsa.lan")))
//     ];

//     return sprinklers;
// }

pub fn get_sprinklers() -> Vec<Box<dyn Sprinkler>> {
    const MASTER_ADDR: &str = "desktop-cyberpower.localdomain:3777";
    let mut builder = SprinklerBuilder::new(SprinklerOptions{ master_addr: String::from(MASTER_ADDR), ..Default::default() });

    // parse FNAME_CONFIG and add triggers
    let sprinklers: Vec<Box<dyn Sprinkler>> = vec![
        Box::new(builder.build::<CommCheck>(String::from("latitude-5289"))),
        Box::new(builder.build::<DockerOOM>(String::from("latitude-5289")))
    ];

    return sprinklers;
}
