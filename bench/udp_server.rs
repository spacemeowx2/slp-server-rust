use bencher::{black_box, Bencher};
use slp_server_rust::test::{make_server, client_connect, make_packet, recv_packet};
use tokio::runtime::{self, Runtime};
use smoltcp::wire::*;

fn relay_n(b: &mut Bencher, count: usize, clinet_count: usize) {
    let mut rt = rt();

    b.iter(|| {
        rt.block_on(async {
            let (_server, addr) = make_server().await;
            let mut sockets = vec![];
            for i in 0..clinet_count {
                let packet = make_packet(
                    Ipv4Address::new(10, 13, 37, 100 + i as u8),
                    Ipv4Address::new(10, 13, 255, 255)
                );
                let mut socket = client_connect(addr).await;
                socket.send(&packet).await.unwrap();
                sockets.push(socket);
            }
            let socket1 = &mut sockets.remove(0);
            let mut socket2 = &mut sockets.remove(0);

            let packet1 = make_packet(
                Ipv4Address::new(10, 13, 37, 100),
                Ipv4Address::new(10, 13, 255, 255)
            );

            for _ in 0..count {
                socket1.send(&packet1).await.unwrap();
                black_box(recv_packet(&mut socket2).await);
            }
        });
    });
}

fn relay_1000(b: &mut Bencher) {
    relay_n(b, 1000, 2)
}

fn relay_2000(b: &mut Bencher) {
    relay_n(b, 2000, 2)
}

fn broadcast_1000_10(b: &mut Bencher) {
    relay_n(b, 1000, 10)
}

fn broadcast_1000_20(b: &mut Bencher) {
    relay_n(b, 1000, 20)
}

fn broadcast_1000_50(b: &mut Bencher) {
    relay_n(b, 1000, 50)
}

fn rt() -> Runtime {
    runtime::Builder::new()
        .threaded_scheduler()
        .core_threads(4)
        .enable_all()
        .build()
        .unwrap()
}

bencher::benchmark_group!(
    relay,
    relay_1000,
    relay_2000
);

bencher::benchmark_group!(
    broadcast,
    broadcast_1000_10,
    broadcast_1000_20,
    broadcast_1000_50
);

bencher::benchmark_main!(relay, broadcast);
