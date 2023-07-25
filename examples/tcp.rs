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
            newfid: Fid(1),
            wnames: vec!["usr", "lib"],
        },
    )?;
    println!("{:?}", res);

    let res = client.send(0, nine_p::TStat { fid: Fid(1) })?;
    println!("{:?}", res);

    let res = client.send(
        0,
        nine_p::TOpen {
            fid: Fid(1),
            mode: 0,
        },
    )?;
    println!("{:?}", res);

    let mut dir_contents = Vec::new(); // XXX
    let mut offset = 0;
    loop {
        let res = client.send(
            0,
            nine_p::TRead {
                fid: Fid(1),
                offset,
                count: 4096,
            },
        )?;
        println!("{:?}", res);
        if res.data.len() == 0 {
            break;
        }
        dir_contents.extend_from_slice(res.data);
        offset += res.data.len() as u64;
    }
    println!("{:?}", nine_p::parse_dir(&dir_contents));

    let res = client.send(0, nine_p::TClunk { fid: Fid(1) })?;
    println!("{:?}", res);

    Ok(())
}
