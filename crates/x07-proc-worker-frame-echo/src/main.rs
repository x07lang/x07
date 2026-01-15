use std::io::{self, Read, Write};

const MAX_FRAME_LEN: usize = 64 * 1024 * 1024;

fn read_exact_or_eof(reader: &mut impl Read, buf: &mut [u8]) -> io::Result<bool> {
    let mut off = 0usize;
    while off < buf.len() {
        let n = reader.read(&mut buf[off..])?;
        if n == 0 {
            if off == 0 {
                return Ok(false);
            }
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "unexpected EOF",
            ));
        }
        off += n;
    }
    Ok(true)
}

fn run() -> io::Result<()> {
    let mut stdin = io::stdin().lock();
    let mut stdout = io::stdout().lock();

    let mut hdr = [0u8; 8];
    loop {
        if !read_exact_or_eof(&mut stdin, &mut hdr)? {
            break;
        }
        let id = u32::from_le_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]);
        let len_u32 = u32::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]);
        let len = usize::try_from(len_u32).unwrap_or(usize::MAX);
        if len > MAX_FRAME_LEN {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "frame too large",
            ));
        }

        let mut payload = vec![0u8; len];
        if len != 0 {
            stdin.read_exact(&mut payload)?;
        }

        stdout.write_all(&id.to_le_bytes())?;
        stdout.write_all(&len_u32.to_le_bytes())?;
        if len != 0 {
            stdout.write_all(&payload)?;
        }
        stdout.flush()?;
    }

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("x07-proc-worker-frame-echo: io error: {e}");
        std::process::exit(1);
    }
}
