#[macro_use]
extern crate old_log;

use value_bag::ValueBag;

pub fn new_log(text: &str) {
    log::info!("new: {}", text);
}

pub fn old_log(text: &str) {
    old_log::info!("old: {}", text);
}

pub fn value_bag() -> ValueBag<'static> {
    ValueBag::capture_display(&42)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_logs() {
        let _ = env_logger::builder().is_test(true).try_init();

        new_log("text");
        old_log("text");
    }

    #[test]
    fn test_value_bag() {
        let bag = value_bag();

        let num = bag.to_u64().unwrap();

        assert_eq!(42, num);
    }
}
