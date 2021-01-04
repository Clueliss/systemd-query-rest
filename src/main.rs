#![feature(decl_macro, array_methods, proc_macro_hygiene)]
#![feature(cstring_from_vec_with_nul)]

extern crate libc;
#[macro_use] extern crate rocket;

use std::ffi::{CStr, OsStr, CString, FromVecWithNulError};
use std::fs::File;
use std::io::{Error, Read};
use std::mem::MaybeUninit;
use std::os::unix::io::FromRawFd;
use std::process::Command;

use rocket::Request;
use rocket::response::Responder;

#[derive(Debug)]
enum ProcessError {
    IOError(std::io::Error),
    OtherError(String)
}

impl <'r> Responder<'r> for ProcessError {
    fn respond_to(self, request: &Request) -> rocket::response::Result<'r> {
        match self {
            ProcessError::IOError(e) => {
                eprintln!("{:?}", e);
                Err(rocket::http::Status::InternalServerError)
            },
            ProcessError::OtherError(e) => e.respond_to(request)
        }
    }
}


impl From<std::io::Error> for ProcessError {
    fn from(e: Error) -> Self {
        ProcessError::IOError(e)
    }
}

fn c_result(ret: libc::c_int) -> Result<libc::c_int, std::io::Error> {
    if ret < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(ret)
    }
}

fn make_c_string<S>(s: S) -> Result<CString, FromVecWithNulError>
where
    S: AsRef<OsStr>
{
    let mut buf = Vec::new();
    let bytes = s.as_ref().to_str().unwrap().as_bytes();
    buf.resize(bytes.len(), 0);
    buf.clone_from_slice(bytes);
    buf.push(0);

    CString::from_vec_with_nul(buf)
}


fn run_command<I, P, A>(prog: P, args: I) -> Result<String, std::io::Error>
where
    P: AsRef<OsStr>,
    A: AsRef<OsStr>,
    I: IntoIterator<Item=A>
{
    unsafe {
        let mut p: [libc::c_int; 2] = MaybeUninit::uninit().assume_init();
        c_result(libc::pipe(p.as_mut_ptr()))?;
        let pid = c_result(libc::fork())?;

        if pid == 0 {
            // in child

            // close read end
            libc::close(p[0]);
            c_result(libc::dup2(p[1], libc::STDOUT_FILENO))?;
            c_result(libc::dup2(p[1], libc::STDERR_FILENO))?;

            let prog = make_c_string(prog).unwrap();
            let args = args.into_iter()
                .map(|arg| make_c_string(arg).unwrap())
                .collect::<Vec<_>>();

            let a = std::iter::once(prog.as_ptr())
                .chain(args.iter().map(|arg| arg.as_ptr()))
                .chain(std::iter::once(std::ptr::null()))
                .collect::<Vec<_>>();

            c_result(libc::execv(prog.as_ptr(), a.as_ptr()))?;
            unreachable!();
        } else {
            // in parent

            // close write end
            libc::close(p[1]);
            c_result(libc::waitpid(pid, std::ptr::null_mut(), 0))?;

            let mut buf = String::new();
            File::from_raw_fd(p[0])
                .read_to_string(&mut buf)?;

            Ok(buf)
        }
    }
}


fn command_output(mut cmd: Command) -> Result<String, ProcessError> {
    let status = cmd.status()?;
    let output = String::from_utf8(cmd.output()?.stdout).unwrap();

    if status.success() {
        Ok(output)
    } else {
        Err(ProcessError::OtherError(output))
    }
}


fn get_systemd_unit_status(unit: &str) -> Result<String, ProcessError> {
    let mut cmd = Command::new("sh");

    cmd.arg("-c")
        .arg(format!("systemctl status '{}' 2>&1", unit));

    command_output(cmd)
}

fn get_systemd_status() -> Result<String, ProcessError> {
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg("systemctl 2>&1");

    command_output(cmd)
}

fn get_systemd_unit_journal(unit: &str, since: Option<&str>) -> Result<String, ProcessError> {
    let mut cmd = Command::new("sh");

    cmd.arg("-c")
        .arg(format!("journalctl --no-pager --unit '{}' 2>&1", unit));

    if since.is_some() {
        cmd.args(&["--since", since.unwrap()]);
    }

    command_output(cmd)
}


#[get("/summary")]
fn summary() -> Result<String, ProcessError> {
    get_systemd_status()
}


#[get("/status/<unit>")]
fn unit_status(unit: String) -> Result<String, ProcessError> {
    get_systemd_unit_status(&unit)
}

#[get("/logs/<unit>?<since>")]
fn unit_logs(unit: String, since: Option<String>) -> Result<String, ProcessError> {
    get_systemd_unit_journal(&unit, since.as_deref())
}


fn main() {
    rocket::ignite()
        .mount("/", routes![summary, unit_status, unit_logs])
        .launch();
}
