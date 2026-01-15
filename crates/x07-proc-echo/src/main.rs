use std::io::{self, Read, Write};

fn run() -> io::Result<()> {
    let mut stdin = io::stdin().lock();
    let mut stdout = io::stdout().lock();

    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = stdin.read(&mut buf)?;
        if n == 0 {
            break;
        }
        stdout.write_all(&buf[..n])?;
    }
    stdout.flush()?;
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("x07-proc-echo: io error: {e}");
        std::process::exit(1);
    }
}
