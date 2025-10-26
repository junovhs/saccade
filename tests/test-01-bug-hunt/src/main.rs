mod api;
mod utils;

fn main() {
    let user_id = 123;
    println!("Fetching data for user {}", user_id);
    let result = api::fetch_user_data(user_id);
    println!("Result: {}", result);
}
