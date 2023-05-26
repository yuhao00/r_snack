use r_snack::Game;

fn main() {
    let mut g = Game::new().unwrap_or_else(|e| {
        println!("初始化游戏失败: {}", e);
        std::process::exit(0);
    });
    if let Err(e) = g.run() {
        println!("fail: {}", e);
    }
    println!("得分: {}", g.score());
}
