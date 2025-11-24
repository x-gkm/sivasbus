use sivasbus::Client;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::new();
    let results = client.get_station_buses(10).await?;
    println!("{results:#?}");

    Ok(())
}
