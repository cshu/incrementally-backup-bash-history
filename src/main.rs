#![allow(clippy::print_literal)]
#![allow(clippy::needless_return)]
#![allow(dropping_references)]
#![allow(clippy::assertions_on_constants)]

use crabrs::*;

use log::*;

use std::path::PathBuf;
use std::process::*;
use std::*;

#[macro_use(defer)]
extern crate scopeguard;

fn main() -> ExitCode {
    env::set_var("RUST_BACKTRACE", "1"); //? not 100% sure this has 0 impact on performance? Maybe setting via command line instead of hardcoding is better?
                                         //env::set_var("RUST_LIB_BACKTRACE", "1");//? this line is useless?
                                         ////
    env::set_var("RUST_LOG", "trace"); //note this line must be above logger init.
    env_logger::init();

    //let args: Vec<String> = env::args().collect(); //Note that std::env::args will panic if any argument contains invalid Unicode.
    fn the_end() {
        if std::thread::panicking() {
            info!("{}", "PANICKING");
        }
        info!("{}", "FINISHED");
    }
    defer! {
        the_end();
    }
    if main_inner(/*args*/).is_err() {
        return ExitCode::from(1);
    }
    ExitCode::from(0)
}

fn main_inner(/*args: Vec<String>*/) -> CustRes<()> {
    //let histsize: u64 = match env::var("HISTSIZE") {
    //    Ok(vstr) => vstr.parse()?,
    //    Err(_) => {
    //        return dummy_err("HISTSIZE not found in env");
    //    }
    //};
    let histfilesize: u64 = match env::var("HISTFILESIZE") {
        Ok(vstr) => vstr.parse()?,
        Err(_) => {
            return dummy_err("HISTFILESIZE not found in env");
        }
    };
    let threshold: String = match env::var("INCR_BACKUP_BASH_HIST_THRESHOLD") {
        Ok(vstr) => vstr,
        Err(_) => {
            coutln!("Using HALF_HISTFILESIZE as default threshold.");
            "HALF_HISTFILESIZE".to_owned()
        }
    };
    let mut con = Ctx {
        //args,
        def: CtxDef::default(),
        //histsize,
        histfilesize,
        threshold,
    };
    con.def.home_dir = dirs::home_dir().ok_or("Failed to get home directory.")?;
    if !real_dir_without_symlink(&con.def.home_dir) {
        return dummy_err("Failed to recognize the home dir as folder.");
    }

    con.def.bash_hist_pb = con.def.home_dir.join(".bash_history");
    con.def.bash_hist_tmp_pb = con.def.home_dir.join(".incr_backup_bash_history_tmp");
    con.def.backup_pb = con.def.home_dir.join(".incr_backup_bash_history");
    match con.threshold.as_str() {
        "HALF_HISTFILESIZE" => {
            try_half_histfilesize_threshold(&mut con)?;
        }
        _ => {
            try_file_size_threshold(&mut con)?;
        }
    }
    Ok(())
}

fn cut_file_at_lf(con: &Ctx, mut bidx: usize) -> CustRes<()> {
    if 0 == bidx {
        return Ok(());
    }
    let num_of_bytes_to_append = bidx + 1;
    use std::io::Write;
    let mut fil = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&con.def.backup_pb)?;
    fil.write_all(num_of_bytes_to_append.to_string().as_bytes())?;
    fil.write_all(b" bytes appended\n")?;
    fil.write_all(&con.def.bytes[..num_of_bytes_to_append])?;
    if bidx != con.def.bytes.len() - 1 {
        bidx += 1; //if bidx is not last byte in file, then cut this byte too
    }
    //fixme there is a very small possibility that .bash_history has changed since you read it. So you should do extra check.

    atomic_write_bash_history(con, bidx)
}

fn try_half_histfilesize_threshold(con: &mut Ctx) -> CustRes<()> {
    if con.histfilesize < 4 {
        return dummy_err("HISTFILESIZE is crazily small.");
    }
    read_bash_history(con)?;
    let mut iterobj = con.def.bytes.iter();
    let mut count = 0;
    while let Some(idx) = iterobj.rposition(|ibyt| *ibyt == b'\n') {
        count += 1;
        if count == con.histfilesize / 2 {
            return cut_file_at_lf(con, idx);
        }
    }
    Ok(())
}

fn try_file_size_threshold(con: &mut Ctx) -> CustRes<()> {
    if !con.threshold.starts_with("FILE_SIZE_") {
        return dummy_err("Invalid INCR_BACKUP_BASH_HIST_THRESHOLD");
    }
    let siz: usize = con.threshold["FILE_SIZE_".len()..].parse()?;
    if siz < 4 {
        return dummy_err("INCR_BACKUP_BASH_HIST_THRESHOLD file size limit is crazily small.");
    }
    read_bash_history(con)?;
    let exceed = if con.def.bytes.len() > siz {
        con.def.bytes.len() - siz
    } else {
        return Ok(());
    };

    let bidx = match con.def.bytes[exceed..].iter().position(|byt| byt == &b'\n') {
        None => {
            return dummy_err("Unable to cut due to file not ending with linefeed");
        }
        Some(inner) => inner + exceed,
    };
    cut_file_at_lf(con, bidx)
}

fn read_bash_history(con: &mut Ctx) -> CustRes<()> {
    con.def.bytes = fs::read(&con.def.bash_hist_pb)?;
    Ok(())
}
fn atomic_write_bash_history(con: &Ctx, bidx: usize) -> CustRes<()> {
    write_bash_history_tmp_file(con, bidx)?;
    fs::rename(&con.def.bash_hist_tmp_pb, &con.def.bash_hist_pb)?;
    //            fs::write(&con.def.bash_hist_pb, &con.def.bytes[bidx..])?;
    Ok(())
}
fn write_bash_history_tmp_file(con: &Ctx, bidx: usize) -> CustRes<()> {
    fs::write(&con.def.bash_hist_tmp_pb, &con.def.bytes[bidx..])?;
    Ok(())
}

struct Ctx {
    //args: Vec<String>,
    def: CtxDef,
    //histsize: u64,
    histfilesize: u64,
    threshold: String,
}

#[derive(Default)]
struct CtxDef {
    home_dir: PathBuf,
    bash_hist_pb: PathBuf,
    bash_hist_tmp_pb: PathBuf,
    backup_pb: PathBuf,
    bytes: Vec<u8>,
}
