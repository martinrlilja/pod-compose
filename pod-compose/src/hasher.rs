use blake3;
use std::hash::{Hash, Hasher};

pub trait DigestHasher {
    fn input<I: Hash>(&mut self, input: I);
}

impl DigestHasher for blake3::Hasher {
    fn input<I: Hash>(&mut self, input: I) {
        struct StdHasher<'a>(&'a mut blake3::Hasher);

        impl<'a> Hasher for StdHasher<'a> {
            fn finish(&self) -> u64 {
                panic!();
            }

            fn write(&mut self, bytes: &[u8]) {
                self.0.update(bytes);
            }
        }

        input.hash(&mut StdHasher(self))
    }
}
