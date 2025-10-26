use crate::utils;

pub fn fetch_user_data(id: u32) -> String {
    let processed_id = utils::process_user(id);
    format!("data_for_{}", processed_id)
}
