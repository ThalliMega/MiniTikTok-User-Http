use mini_tiktok_user_http::{block_on, start_up};

fn main() {
    block_on(start_up().unwrap()).unwrap().unwrap().unwrap()
}
