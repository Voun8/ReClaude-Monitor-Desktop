// 固定端口转发器：在 reclaude daemon（动态端口）前面架一个稳定端口。
//
// happy()/claude 把 HTTPS_PROXY 指向这个固定端口；daemon 重启换端口也不影响——
// 每个新连接都现读 state.json 的“当前” daemon 端口再转发。于是切换账号后，
// 正在跑的会话重试时会重连到固定端口 → 落到新 daemon → 新账号，实现热切换。
//
// 纯字节双向管道，不解析 HTTP/TLS（客户端的 CONNECT 由 daemon 处理）。

use std::io;
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use crate::switcher;

/// 对外暴露的稳定代理端口。选个冷门高端口，避开 reclaude 动态端口（5xxxx）。
pub const FIXED_PORT: u16 = 47600;

/// 启动后台转发线程。端口被占用 / 无权限等失败时安静返回——
/// happy() 检测不到固定端口会自动回退动态端口，绝不影响 App 启动和既有用法。
pub fn spawn() {
    thread::spawn(|| {
        let listener = match TcpListener::bind(("127.0.0.1", FIXED_PORT)) {
            Ok(l) => l,
            Err(e) => {
                // 设计上的安静回退（happy() 会改用动态端口），留一行日志便于区分「回退」与「环境异常」
                eprintln!("[forwarder] 固定端口 {FIXED_PORT} 监听失败，回退动态端口: {e}");
                return;
            }
        };
        for stream in listener.incoming() {
            if let Ok(client) = stream {
                thread::spawn(move || handle(client));
            }
        }
    });
}

fn handle(client: TcpStream) {
    // 现读“当前” daemon 端口——这正是热切换的关键：切换后新连接自动落到新 daemon。
    let port = match switcher::Paths::resolve() {
        Ok(paths) => switcher::read_daemon_port(&paths),
        Err(_) => return,
    };
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let upstream = match TcpStream::connect_timeout(&addr, Duration::from_millis(800)) {
        Ok(s) => s,
        Err(_) => return, // daemon 正在重启 / 未监听：断开，客户端会重试
    };

    let _ = client.set_nodelay(true);
    let _ = upstream.set_nodelay(true);

    let mut c_read = match client.try_clone() {
        Ok(c) => c,
        Err(_) => return,
    };
    let mut c_write = client;
    let mut u_read = match upstream.try_clone() {
        Ok(u) => u,
        Err(_) => return,
    };
    let mut u_write = upstream;

    // client → upstream（独立线程）
    let t = thread::spawn(move || {
        let _ = io::copy(&mut c_read, &mut u_write);
        let _ = u_write.shutdown(Shutdown::Write);
    });
    // upstream → client（本线程）
    let _ = io::copy(&mut u_read, &mut c_write);
    let _ = c_write.shutdown(Shutdown::Write);
    let _ = t.join();
}
