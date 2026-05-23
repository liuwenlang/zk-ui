use std::time::Duration;
use zookeeper::{Acl, CreateMode, ZooKeeper, Watcher};

struct Nop;
impl Watcher for Nop { fn handle(&self, _: zookeeper::WatchedEvent) {} }

fn main() {
    let zk = ZooKeeper::connect("127.0.0.1:2181", Duration::from_secs(5), Nop).unwrap();
    let acl = vec![Acl::new(zookeeper::Permission::ALL, "world", "anyone")];

    if zk.exists("/nihao", false).unwrap().is_none() {
        zk.create("/nihao", vec![], acl.clone(), CreateMode::Persistent).unwrap();
    }

    let mut ok = 0u32;
    for i in 0..1000u32 {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let suffix = (ts & 0xFFFF) as u16;
        let name = format!("/nihao/node-{:04}-{:04x}", i, suffix);
        let data = format!("test-data-{}", i);
        match zk.create(&name, data.as_bytes().to_vec(), acl.clone(), CreateMode::Persistent) {
            Ok(_) => {
                ok += 1;
                if ok % 100 == 0 { println!("{}/1000", ok); }
            }
            Err(e) => eprintln!("fail: {} {}", name, e),
        }
    }
    println!("done, created {}", ok);
}
