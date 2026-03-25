// effects.mg — effect definitions, handlers, closures

effect io {
    def read(fd: i32) -> Vec<u8>;
    def write(fd: i32, data: &[u8]) -> i32;
}

effect async {
    def suspend() -> ();
}

exp def process_data(input: &[u8]) -> Result<i32, Error> {
    val result = 0;
    each byte of input {
        when byte > 127 {
            emit Result::Err(Error::new("invalid byte"));
        }
        result = result + byte;
    }
    Result::Ok(result)
}

def transform<T, U>(items: Vec<T>, mapper: def(T) -> U) -> Vec<U> {
    val out = Vec::new();
    each item of items {
        out.push(mapper(item));
    }
    out
}

def example() {
    val doubled = transform(vec![1, 2, 3], |x| x * 2);
    val filtered = transform(doubled, |x| x + 1);
}
