use std::net::TcpStream;

fn main() -> Result<(), nine_p::Error> {
    let stream = TcpStream::connect("localhost:564")?;
    let mut client = nine_p::SyncClient::new(stream);
    let res = client.send(
        65535,
        nine_p::TVersion {
            msize: 8192,
            version: "9P2000",
        },
    )?;
    println!("{:?}", res);
    Ok(())
}
