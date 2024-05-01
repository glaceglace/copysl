use std::{env, fs};
use std::fs::File;
use std::path::PathBuf;

pub fn daemonize() {
//TODO optimize output_dir for macOS
    let home_folder = env::var("HOME").unwrap_or_else(|err| {
        panic!("Failed to get home directory, nested error is {}", err);
    });
    let mut output_dir = home_folder.to_string() + "/.copysl/";
    output_dir = "/tmp/".to_string();
    if fs::metadata(&output_dir).is_err() {
        fs::create_dir(&output_dir).unwrap_or_else(|err| {
            panic!("Failed to create directory, nested error is {}", err)
        });
    }

    let mut output_dir = PathBuf::from(&output_dir);
    output_dir.push("copysl.log");
    let stdout = File::create(&output_dir);
    output_dir.pop();
    output_dir.push("copysl.err");
    let stderr = File::create(&output_dir);

    output_dir.pop();
    output_dir.push("copysl.pid");
    let pid_dir = output_dir;


    let daemonize = daemonize::Daemonize::new()
        .pid_file(pid_dir)
        // .chown_pid_file(true)
        // .working_directory("~/.copysl")
        // .user("nb")
        // .group("nb")
        // .umask(0o777)
        .stdout(stdout.unwrap())
        .stderr(stderr.unwrap())
        .privileged_action(|| "Executed before drop privileges");

    let damemon_start = daemonize.start();
    match damemon_start {
        Ok(_) => { println!("Daemon started") }
        Err(err) => { panic!("Failed to start copysl, nested error is: {}", err) }
    }
}
