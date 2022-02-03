use ssup::Client;
use ssup::UploadLine;

#[tokio::main]
async fn main() {
    let client = Client::new(UploadLine::kodo());
    client.upload(&[]).await;
}
