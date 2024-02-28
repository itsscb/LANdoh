use landoh::server::ServerOld;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // let _ = read_files("testdir");

    let addr = "0.0.0.0:9001".parse()?;
    let sv = ServerOld::default();
    sv.serve(addr).await?;

    Ok(())
}
