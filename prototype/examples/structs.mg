// structs.mg — records, sums, extensions, generics

exp rec Point<T> {
    exp x: T,
    exp y: T,
}

sum Shape {
    Circle(f64),
    Rect(f64, f64),
    Poly(Vec<Point<f64>>),
}

ext Point<T: Copy> {
    exp def distance(&self, other: &Point<T>) -> f64 {
        val dx = self.x - other.x;
        val dy = self.y - other.y;
        (dx * dx + dy * dy)
    }
}

exp def make_origin() -> Point<f64> {
    val p = Point { x: 0.0, y: 0.0 };
    p
}
