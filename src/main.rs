#![feature(decl_macro, array_methods, proc_macro_hygiene)]

#[macro_use] extern crate rocket;

use std::io::Error;
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


fn command_output(mut cmd: Command) -> Result<String, ProcessError> {
    let status = cmd.status()?;
    let output = cmd.output()?;

    if !status.success() {
        Err(ProcessError::OtherError(String::from_utf8(output.stderr).unwrap()))
    } else {
        Ok(String::from_utf8(output.stdout).unwrap())
    }
}


fn get_systemd_unit_status(unit: &str) -> Result<String, ProcessError> {
    let mut cmd = Command::new("systemctl");

    cmd.arg("status")
        .arg(unit);

    command_output(cmd)
}

fn get_systemd_status() -> Result<String, ProcessError> {
    command_output(Command::new("systemctl"))
}

fn get_systemd_unit_journal(unit: &str, since: Option<&str>) -> Result<String, ProcessError> {
    let mut cmd = Command::new("journalctl");

    cmd.arg("--unit")
        .arg(unit);

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