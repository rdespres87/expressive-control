#![no_std]

pub fn test_core() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        assert!(test_core());
    }
}
