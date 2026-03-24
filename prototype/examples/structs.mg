// structs.mg — structs, enums, impl blocks, generics

pub struct Point<T> {
    pub x: T,
    pub y: T,
}

enum Shape {
    Circle(f64),
    Rect(f64, f64),
    Poly(Vec<Point<f64>>),
}

impl Point<T: Copy> {
    pub fn distance(&self, other: &Point<T>) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy)
    }
}

pub fn make_origin() -> Point<f64> {
    let p = Point { x: 0.0, y: 0.0 };
    p
}
