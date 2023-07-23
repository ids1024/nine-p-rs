use std::net::TcpStream;

use nine_p::Fid;

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

    let res = client.send(
        0,
        nine_p::TAuth {
            afid: Fid(0),
            uname: "foo",
            aname: "bar",
        },
    );
    println!("{:?}", res);

    let res = client.send(
        0,
        nine_p::TAttach {
            fid: Fid(0),
            afid: Fid(u32::MAX),
            uname: "foo",
            aname: "bar",
        },
    )?;
    println!("{:?}", res);

    let res = client.send(
        0,
        nine_p::TWalk {
            fid: Fid(0),
            newfid: Fid(0),
            wnames: vec!["usr", "lib"],
        },
    )?;
    println!("{:?}", res);

    Ok(())
}
