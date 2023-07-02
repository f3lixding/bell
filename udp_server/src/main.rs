use tokio::net::UdpSocket;
use lib_udp_server::Point;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Program started");
    let socket = UdpSocket::bind("127.0.0.1:8080").await?;
    let mut buf = vec![0; 1024];

    while let Ok((size, src)) = socket.recv_from(&mut buf).await {
        let data = &buf[..size];
        println!("Received {} bytes from {}", size, src);
        let mut point = serde_json::from_slice::<Point>(&data).unwrap_or_default();
        println!("Received a point: {:?}", point);
        if point.x > 220.0 {
            point.x = 120.0;
            let point_byte = serde_json::to_vec(&point).unwrap();
            socket.send_to(&point_byte, src).await?;
        }
//         point_byte.push("\n".as_bytes()[0]);
//         socket.send_to(&point_byte, src).await?;
    }

    Ok(())
}
