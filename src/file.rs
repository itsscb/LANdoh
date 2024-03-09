use std::{
    error::Error,
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    net::SocketAddr,
    path::PathBuf,
    pin::Pin,
};

use tokio::sync::mpsc::{self, Sender};
use tonic::Status;

use data_encoding::HEXUPPER;
use ring::digest::{Context, Digest, SHA256};
mod model {
    include!("model.rs");
}

use self::model::{Directory, CHUNK_SIZE};

fn sha256_digest<R: Read>(mut reader: R) -> Result<String, Box<dyn Error>> {
    let mut context = Context::new(&SHA256);
    let mut buffer = [0; CHUNK_SIZE];

    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        context.update(&buffer.clone()[..count]);
    }
    Ok(HEXUPPER.encode(context.finish().as_ref()))
}

#[test]
fn test_hash() {
    let path = "testdir/file_root";

    let input = File::open(path).expect("failed to open file");
    let reader = BufReader::new(input);
    let digest = sha256_digest(reader).expect("failed to digest file");

    println!("SHA-256 digest is {}", digest);
}
