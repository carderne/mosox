use crate::data::Entry;

pub fn print_entries(entries: &Vec<Entry>) {
    for d in entries {
        println!("{d}")
    }
}
