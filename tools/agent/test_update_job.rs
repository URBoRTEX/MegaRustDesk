// Real Windows jobs and child handles, no product GUI/data or external processes.
#[path="../../src/agent/process_job.rs"] mod process_job;
use std::{fs,io::{Read,Write},path::Path,process::{Command,Stdio},os::windows::{process::CommandExt,io::AsRawHandle},thread,time::{Duration,Instant}};
use winapi::um::{jobapi2::*,jobapi::IsProcessInJob,winnt::*,handleapi::CloseHandle,processthreadsapi::OpenProcess,synchapi::WaitForSingleObject};
fn wait_file(p:&Path) {let t=Instant::now();while !p.exists(){assert!(t.elapsed()<Duration::from_secs(8),"file timeout {p:?}");thread::sleep(Duration::from_millis(20));}}
fn main() {
    let args:Vec<_>=std::env::args().collect();
    if args.get(1).map(String::as_str)==Some("probe") {
        let p=Path::new(&args[2]);fs::write(p,std::process::id().to_string()).unwrap();
        let t=Instant::now();while !p.with_extension("stop").exists() && t.elapsed()<Duration::from_secs(15){thread::sleep(Duration::from_millis(20));}return;
    }
    if args.get(1).map(String::as_str)==Some("worker") {
        let root=Path::new(&args[2]);let mut start=[0];std::io::stdin().read_exact(&mut start).unwrap();
        unsafe {process_job::create().unwrap();}
        let exe=std::env::current_exe().unwrap();
        let normal=Command::new(&exe).arg("probe").arg(root.join("normal.pid")).creation_flags(0x08000000).spawn().unwrap();
        assert_eq!(process_job::spawn_flags(),0x09000000);
        let helper=Command::new(&exe).arg("probe").arg(root.join("helper.pid")).creation_flags(process_job::spawn_flags()).spawn().unwrap();
        let rejected_normal=process_job::check_child(&normal).is_err();
        let allowed_helper=process_job::check_child(&helper).is_ok();
        let mut any=0;unsafe{assert_ne!(IsProcessInJob(helper.as_raw_handle() as _,std::ptr::null_mut(),&mut any),0);}
        fs::write(root.join("checks.txt"),format!("{rejected_normal},{allowed_helper},{}",any!=0)).unwrap();
        wait_file(&root.join("exit"));return;
    }
    let base=std::env::current_dir().unwrap().join(".agent-build");let root=base.join(format!("job-lifecycle-{}",std::process::id()));fs::create_dir(&root).unwrap();
    let outer=unsafe{let h=CreateJobObjectW(std::ptr::null_mut(),std::ptr::null());assert!(!h.is_null());let mut l:JOBOBJECT_EXTENDED_LIMIT_INFORMATION=std::mem::zeroed();l.BasicLimitInformation.LimitFlags=JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;assert_ne!(SetInformationJobObject(h,JobObjectExtendedLimitInformation,&mut l as *mut _ as _,std::mem::size_of_val(&l) as u32),0);h};
    assert_eq!(process_job::spawn_flags(),0x08000000);
    let mut worker=Command::new(std::env::current_exe().unwrap()).arg("worker").arg(&root).stdin(Stdio::piped()).creation_flags(process_job::spawn_flags()).spawn().unwrap();
    process_job::check_child(&worker).unwrap();
    unsafe{assert_ne!(AssignProcessToJobObject(outer,worker.as_raw_handle() as _),0);}
    worker.stdin.take().unwrap().write_all(&[1]).unwrap();
    wait_file(&root.join("checks.txt"));wait_file(&root.join("normal.pid"));wait_file(&root.join("helper.pid"));
    let checks=fs::read_to_string(root.join("checks.txt")).unwrap();
    let handles:Vec<_>=["normal.pid","helper.pid"].into_iter().map(|name|{let pid=fs::read_to_string(root.join(name)).unwrap().parse().unwrap();unsafe{let h=OpenProcess(SYNCHRONIZE|PROCESS_QUERY_LIMITED_INFORMATION,0,pid);assert!(!h.is_null());h}}).collect();
    fs::write(root.join("exit"),"").unwrap();worker.wait().unwrap();
    let normal_dead=unsafe{WaitForSingleObject(handles[0],2000)==0};let helper_alive=unsafe{WaitForSingleObject(handles[1],100)==258};
    fs::write(root.join("helper.stop"),"").unwrap();unsafe{assert_eq!(WaitForSingleObject(handles[1],2000),0);for h in handles{CloseHandle(h);}CloseHandle(outer);}
    println!("membership(normal rejected, helper allowed, helper in outer job)={checks}; normal exits={normal_dead}; helper survives={helper_alive}");
    assert_eq!(checks,"true,true,true","external job is not the application job");assert!(normal_dead && helper_alive);
}
