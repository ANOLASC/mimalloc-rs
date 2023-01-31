mod alloc;
mod arena;
mod init;
mod mimalloc_internal;
mod mimalloc_types;
mod os;
mod page;
mod segment;

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
