// sample-tree/src/main.rs

use clap::{command, Parser};
use libc::{proc_bsdinfo, proc_pidinfo, PROC_PIDTBSDINFO};
use psutil::process::{Process, ProcessCollector, ProcessError, ProcessResult};
use std::collections::HashSet;
use std::ffi::c_int;
use std::fs;
use std::mem;
use std::panic;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The output directory to write samples to (will be created if necessary)
    #[arg(short, long, required = true)]
    output_dir: PathBuf,
    /// The command to profile
    #[arg(last = true, required = true)]
    command: Vec<String>,
}

fn main() {
    let args = Args::parse();
    let mut command = Command::new(&args.command[0]);
    command.args(&args.command[1..]);

    let root_child = Arc::new(Mutex::new(command.spawn().unwrap()));
    sample(
        &args,
        root_child.lock().unwrap().id(),
        &PathBuf::from(&args.command[0])
            .file_name()
            .unwrap_or_default()
            .to_string_lossy(),
    );

    // Kill the child if we die to avoid annoyance
    let old_hook = panic::take_hook();
    let root_child_2 = root_child.clone();
    panic::set_hook(Box::new(move |info| {
        drop(root_child_2.lock().unwrap().kill());
        old_hook(info)
    }));

    let root_pid = root_child.lock().unwrap().id();
    let Ok(root_process) = Process::new(root_pid) else { return };

    drop(fs::create_dir(&args.output_dir));

    let mut processes = ProcessCollector::new().unwrap();
    processes.update().unwrap();

    let mut monitoring = HashSet::new();
    monitoring.insert(root_pid);

    while root_child
        .lock()
        .unwrap()
        .try_wait()
        .map_or(false, |status| status.is_none())
    {
        drop(processes.update());
        for (&pid, process) in &processes.processes {
            if !monitoring.contains(&pid) && is_descendant_of(process, &root_process) {
                sample(&args, pid, &process.name().unwrap_or_default());
                monitoring.insert(pid);
            }
        }

        thread::sleep(Duration::from_millis(1000));
    }
}

fn sample(args: &Args, pid: u32, name: &str) {
    let mut out_path = args.output_dir.clone();
    out_path.push(&format!("{}-{}.txt", name, pid));
    let mut command = Command::new("sample");
    command.args([
        "-mayDie",
        "-file",
        &out_path.to_string_lossy(),
        &pid.to_string(),
    ]);
    thread::spawn(move || command.spawn().unwrap().wait());
}

fn is_descendant_of(kid: &Process, ancestor: &Process) -> bool {
    kid.pid() == ancestor.pid()
        || process_parent(kid).map_or(false, |parent_opt| {
            parent_opt.map_or(false, |parent| is_descendant_of(&parent, ancestor))
        })
}

fn process_parent(kid: &Process) -> ProcessResult<Option<Process>> {
    unsafe {
        let mut proc_info: proc_bsdinfo = mem::zeroed();
        if proc_pidinfo(
            kid.pid() as c_int,
            PROC_PIDTBSDINFO,
            0,
            &mut proc_info as *mut _ as *mut _,
            mem::size_of::<proc_bsdinfo>() as c_int,
        ) < 0
        {
            return Err(ProcessError::NoSuchProcess { pid: kid.pid() });
        }
        if proc_info.pbi_ppid == 0 || proc_info.pbi_ppid == kid.pid() {
            Ok(None)
        } else {
            Ok(Some(Process::new(proc_info.pbi_ppid)?))
        }
    }
}
