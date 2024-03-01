use landoh::server::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "0.0.0.0:9001".parse()?;
    let sv = Server::default();
    sv.serve(addr).await?;

    Ok(())
}
