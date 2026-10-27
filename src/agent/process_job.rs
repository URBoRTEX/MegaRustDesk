//! Only our portable application's job; external Windows job policy is preserved.
use std::{process::Child, os::windows::io::AsRawHandle, sync::atomic::{AtomicIsize, Ordering}};
use winapi::um::{jobapi::IsProcessInJob, jobapi2::*, processthreadsapi::GetCurrentProcess, winnt::*};
static OWN_JOB: AtomicIsize = AtomicIsize::new(0);

pub fn spawn_flags() -> u32 {
    // Recovery/offline repair has no owned job to leave; keep all external limits.
    0x08000000 | if OWN_JOB.load(Ordering::Acquire)!=0 { 0x01000000 } else { 0 }
}

pub unsafe fn create() -> Result<(), String> {
    let job=CreateJobObjectW(std::ptr::null_mut(),std::ptr::null());
    if job.is_null() {return Err("Не создан контейнер процессов".into());}
    let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION=std::mem::zeroed();
    limits.BasicLimitInformation.LimitFlags=JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE|JOB_OBJECT_LIMIT_BREAKAWAY_OK;
    if SetInformationJobObject(job,JobObjectExtendedLimitInformation,&mut limits as *mut _ as _,std::mem::size_of_val(&limits) as u32)==0
        || AssignProcessToJobObject(job,GetCurrentProcess())==0 {
        winapi::um::handleapi::CloseHandle(job);
        return Err(format!("Не включено завершение дочерних процессов: {}",std::io::Error::last_os_error()));
    }
    // Non-inheritable handle, held until process exit. Never pass it to the helper.
    OWN_JOB.store(job as isize,Ordering::Release);
    Ok(())
}

pub fn check_child(child: &Child) -> Result<(), String> {
    let job=OWN_JOB.load(Ordering::Acquire) as winapi::shared::ntdef::HANDLE;
    // Startup recovery runs before an application-owned job exists.
    if job.is_null() { return Ok(()); }
    let mut inside=0;
    unsafe {
        if IsProcessInJob(child.as_raw_handle() as _,job,&mut inside)==0 || inside!=0 {
            return Err("Updater не вышел из контейнера приложения. Текущая версия сохранена".into());
        }
    }
    Ok(())
}
